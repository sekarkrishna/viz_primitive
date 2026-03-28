// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Krishnamoorthy Sankaran <krishnamoorthy.sankaran@sekrad.org>

//! ParquetLoader — reads parquet files into Arrow RecordBatch and extracts f32 column pairs.

use std::fs::File;
use std::path::{Path, PathBuf};

use arrow::array::{Array, AsArray, Float32Array};
use arrow::datatypes::DataType;
use arrow::record_batch::RecordBatch;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use thiserror::Error;

/// A pair of equal-length f32 vectors extracted from parquet columns.
#[derive(Debug, Clone)]
pub struct ColumnPair {
    /// X values.
    pub x: Vec<f32>,
    /// Y values.
    pub y: Vec<f32>,
}

/// Parquet loading errors.
#[derive(Debug, Error)]
pub enum ParquetError {
    /// IO error.
    #[error("IO error for path '{path}': {source}")]
    Io {
        /// Path to the file that caused the error.
        path: PathBuf,
        /// The underlying IO error.
        source: std::io::Error,
    },
    /// Column not found.
    #[error("Column '{column}' not found. Available columns: {available:?}")]
    MissingColumn {
        /// Name of the missing column.
        column: String,
        /// List of available column names.
        available: Vec<String>,
    },
    /// Column is not numeric.
    #[error("Column '{column}' is not numeric (actual type: {actual_type})")]
    NonNumericColumn {
        /// Name of the non-numeric column.
        column: String,
        /// The actual data type of the column.
        actual_type: String,
    },
    /// Arrow error.
    #[error("Arrow error: {0}")]
    ArrowError(String),
}

/// Parquet file loader.
pub struct ParquetLoader;

impl ParquetLoader {
    /// Stream a Parquet file row-group by row-group, extracting columns into a ColumnPair.
    pub fn load_columns(path: &Path, x_col: &str, y_col: &str) -> Result<ColumnPair, ParquetError> {
        let file = File::open(path).map_err(|e| ParquetError::Io { path: path.to_path_buf(), source: e })?;
        let builder = ParquetRecordBatchReaderBuilder::try_new(file)
            .map_err(|e| ParquetError::ArrowError(e.to_string()))?;

        let schema = builder.schema();
        let available: Vec<String> = schema.fields().iter().map(|f| f.name().clone()).collect();
        if schema.index_of(x_col).is_err() {
            return Err(ParquetError::MissingColumn { column: x_col.to_string(), available: available.clone() });
        }
        if schema.index_of(y_col).is_err() {
            return Err(ParquetError::MissingColumn { column: y_col.to_string(), available });
        }

        let reader = builder.build().map_err(|e| ParquetError::ArrowError(e.to_string()))?;
        let mut x_vals = Vec::new();
        let mut y_vals = Vec::new();
        let mut found_any = false;

        for batch_result in reader {
            let batch = batch_result.map_err(|e| ParquetError::ArrowError(e.to_string()))?;
            found_any = true;
            let x_idx = batch.schema().index_of(x_col).map_err(|_| ParquetError::ArrowError(format!("column '{x_col}' missing in batch")))?;
            let y_idx = batch.schema().index_of(y_col).map_err(|_| ParquetError::ArrowError(format!("column '{y_col}' missing in batch")))?;
            let x_f32 = cast_to_f32(batch.column(x_idx), x_col)?;
            let y_f32 = cast_to_f32(batch.column(y_idx), y_col)?;
            for i in 0..x_f32.len() {
                if x_f32.is_valid(i) && y_f32.is_valid(i) {
                    x_vals.push(x_f32.value(i));
                    y_vals.push(y_f32.value(i));
                }
            }
        }

        if !found_any {
            return Err(ParquetError::ArrowError("Parquet file contains no record batches".into()));
        }
        Ok(ColumnPair { x: x_vals, y: y_vals })
    }

    /// Read a parquet file into a single Arrow RecordBatch.
    pub fn load(path: &Path) -> Result<RecordBatch, ParquetError> {
        let file = File::open(path).map_err(|e| ParquetError::Io { path: path.to_path_buf(), source: e })?;
        let builder = ParquetRecordBatchReaderBuilder::try_new(file).map_err(|e| ParquetError::ArrowError(e.to_string()))?;
        let reader = builder.build().map_err(|e| ParquetError::ArrowError(e.to_string()))?;
        let mut batches = Vec::new();
        for batch_result in reader {
            batches.push(batch_result.map_err(|e| ParquetError::ArrowError(e.to_string()))?);
        }
        if batches.is_empty() { return Err(ParquetError::ArrowError("Parquet file contains no record batches".into())); }
        if batches.len() == 1 { return Ok(batches.into_iter().next().unwrap()); }
        arrow::compute::concat_batches(&batches[0].schema(), &batches).map_err(|e| ParquetError::ArrowError(e.to_string()))
    }

    /// Extract named x/y columns from a RecordBatch as f32 arrays.
    pub fn extract_columns(batch: &RecordBatch, x_col: &str, y_col: &str) -> Result<ColumnPair, ParquetError> {
        let schema = batch.schema();
        let available: Vec<String> = schema.fields().iter().map(|f| f.name().clone()).collect();
        let x_idx = schema.index_of(x_col).map_err(|_| ParquetError::MissingColumn { column: x_col.to_string(), available: available.clone() })?;
        let y_idx = schema.index_of(y_col).map_err(|_| ParquetError::MissingColumn { column: y_col.to_string(), available })?;
        let x_f32 = cast_to_f32(batch.column(x_idx), x_col)?;
        let y_f32 = cast_to_f32(batch.column(y_idx), y_col)?;
        let num_rows = x_f32.len();
        let mut x_vals = Vec::with_capacity(num_rows);
        let mut y_vals = Vec::with_capacity(num_rows);
        for i in 0..num_rows {
            if x_f32.is_valid(i) && y_f32.is_valid(i) {
                x_vals.push(x_f32.value(i));
                y_vals.push(y_f32.value(i));
            }
        }
        Ok(ColumnPair { x: x_vals, y: y_vals })
    }
}

fn cast_to_f32(array: &dyn arrow::array::Array, col_name: &str) -> Result<Float32Array, ParquetError> {
    match array.data_type() {
        DataType::Float32 => Ok(array.as_primitive::<arrow::datatypes::Float32Type>().clone()),
        DataType::Float64 => {
            let arr = array.as_primitive::<arrow::datatypes::Float64Type>();
            Ok(arr.iter().map(|v| v.map(|val| val as f32)).collect())
        }
        DataType::Int32 => {
            let arr = array.as_primitive::<arrow::datatypes::Int32Type>();
            Ok(arr.iter().map(|v| v.map(|val| val as f32)).collect())
        }
        DataType::Int64 => {
            let arr = array.as_primitive::<arrow::datatypes::Int64Type>();
            Ok(arr.iter().map(|v| v.map(|val| val as f32)).collect())
        }
        other => Err(ParquetError::NonNumericColumn { column: col_name.to_string(), actual_type: format!("{other:?}") }),
    }
}
