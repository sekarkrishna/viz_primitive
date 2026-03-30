mod windowed;

use numpy::{IntoPyArray, PyArray1, PyArray3, PyReadonlyArray1, PyReadonlyArray2};
use pyo3::prelude::*;
use ndarray::Array3;
use std::path::Path;

use ::dr2d::headless::HeadlessRenderer as RustHeadlessRenderer;
use ::dr2d::renderer::sdf_pipeline::SdfInstance;
use ::dr2d::data::parquet_loader::{ParquetLoader, ParquetError};
use ::dr2d::data::coord_mapper::{
    CoordinateMapper as RustCoordinateMapper,
    DataRange,
};

use winit::event_loop::EventLoop;

#[pyclass]
struct HeadlessRenderer {
    inner: RustHeadlessRenderer,
}

#[pymethods]
impl HeadlessRenderer {
    #[new]
    fn new() -> PyResult<Self> {
        let inner = pollster::block_on(RustHeadlessRenderer::new())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(HeadlessRenderer { inner })
    }

    /// Render SDF instances to a numpy RGBA array.
    ///
    /// instances: Nx10 float32 array with columns:
    ///   [position_x, position_y, size_x, size_y, r, g, b, a, shape_type, param]
    ///
    /// Returns: numpy uint8 array of shape (height, width, 4).
    #[pyo3(signature = (instances, width, height))]
    fn render_sdf_to_numpy<'py>(
        &mut self,
        py: Python<'py>,
        instances: PyReadonlyArray2<f32>,
        width: u32,
        height: u32,
    ) -> PyResult<&'py PyArray3<u8>> {
        // Validate dimensions
        if width == 0 || height == 0 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "width and height must be > 0",
            ));
        }

        let arr = instances.as_array();
        let shape = arr.shape();

        // Validate column count
        if shape.len() != 2 || shape[1] != 10 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "Instances array must be shape (N, 10)",
            ));
        }

        let num_instances = shape[0];
        let mut sdf_instances = Vec::with_capacity(num_instances);

        for row in arr.rows() {
            sdf_instances.push(SdfInstance {
                position: [row[0], row[1]],
                size: [row[2], row[3]],
                color: [row[4], row[5], row[6], row[7]],
                shape_type: row[8] as u32,
                param: row[9],
                _pad: [0.0, 0.0],
            });
        }

        let pixels = pollster::block_on(
            self.inner
                .render_sdf_to_image(width, height, &sdf_instances),
        )
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        // Convert flat Vec<u8> to 3D (height, width, 4) array
        let py_array =
            Array3::from_shape_vec((height as usize, width as usize, 4), pixels)
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(py_array.into_pyarray(py))
    }
}

#[pyclass]
struct CoordinateMapper {
    inner: RustCoordinateMapper,
}

#[pymethods]
impl CoordinateMapper {
    #[new]
    #[pyo3(signature = (x_min, x_max, y_min, y_max, scene_width, scene_height))]
    fn new(
        x_min: f32,
        x_max: f32,
        y_min: f32,
        y_max: f32,
        scene_width: f32,
        scene_height: f32,
    ) -> Self {
        let inner = RustCoordinateMapper {
            x_data_range: DataRange { min: x_min, max: x_max },
            y_data_range: DataRange { min: y_min, max: y_max },
            x_scene_range: (0.0, scene_width),
            y_scene_range: (0.0, scene_height),
        };
        CoordinateMapper { inner }
    }

    /// Map data coordinates to scene coordinates.
    ///
    /// x, y: float32 numpy arrays of equal length.
    /// Returns (scene_x, scene_y) as a tuple of float32 numpy arrays.
    #[pyo3(signature = (x, y))]
    fn map_points<'py>(
        &self,
        py: Python<'py>,
        x: PyReadonlyArray1<f32>,
        y: PyReadonlyArray1<f32>,
    ) -> PyResult<(&'py PyArray1<f32>, &'py PyArray1<f32>)> {
        let x_arr = x.as_array();
        let y_arr = y.as_array();

        let len = x_arr.len();
        if len != y_arr.len() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "x and y arrays must have the same length",
            ));
        }

        let mut scene_x = Vec::with_capacity(len);
        let mut scene_y = Vec::with_capacity(len);

        for i in 0..len {
            let (sx, sy) = self.inner.map_point(x_arr[i], y_arr[i]);
            scene_x.push(sx);
            scene_y.push(sy);
        }

        Ok((scene_x.into_pyarray(py), scene_y.into_pyarray(py)))
    }
}

/// Load two columns from a parquet file as numpy float32 arrays.
///
/// Returns (x_array, y_array).
///
/// Raises:
///   FileNotFoundError — if the file does not exist
///   ValueError        — if a requested column is missing (message lists available columns)
///   TypeError         — if a requested column is not numeric
#[pyfunction]
#[pyo3(signature = (path, x_col, y_col))]
fn load_parquet_columns<'py>(
    py: Python<'py>,
    path: &str,
    x_col: &str,
    y_col: &str,
) -> PyResult<(&'py PyArray1<f32>, &'py PyArray1<f32>)> {
    let pair = ParquetLoader::load_columns(Path::new(path), x_col, y_col)
        .map_err(|e| match &e {
            ParquetError::Io { .. } => {
                pyo3::exceptions::PyFileNotFoundError::new_err(e.to_string())
            }
            ParquetError::MissingColumn { .. } => {
                pyo3::exceptions::PyValueError::new_err(e.to_string())
            }
            ParquetError::NonNumericColumn { .. } => {
                pyo3::exceptions::PyTypeError::new_err(e.to_string())
            }
            _ => pyo3::exceptions::PyRuntimeError::new_err(e.to_string()),
        })?;

    let x_array = pair.x.into_pyarray(py);
    let y_array = pair.y.into_pyarray(py);
    Ok((x_array, y_array))
}

/// Open an interactive window displaying SDF instances with pan/zoom support.
///
/// instances: Nx10 float32 array with columns:
///   [position_x, position_y, size_x, size_y, r, g, b, a, shape_type, param]
///
/// Blocks until the window is closed. Releases the GIL while the event loop runs.
#[pyfunction]
#[pyo3(signature = (instances, width, height, title="dr2d"))]
fn show_sdf_window(
    py: Python<'_>,
    instances: PyReadonlyArray2<f32>,
    width: u32,
    height: u32,
    title: &str,
) -> PyResult<()> {
    // Validate dimensions
    if width == 0 || height == 0 {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "width and height must be > 0",
        ));
    }

    let arr = instances.as_array();
    let shape = arr.shape();

    // Validate column count
    if shape.len() != 2 || shape[1] != 10 {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "Instances array must be shape (N, 10)",
        ));
    }

    let num_instances = shape[0];
    if num_instances == 0 {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "Instances array must not be empty",
        ));
    }

    // Convert numpy rows to Vec<SdfInstance> and extract scene points
    let mut sdf_instances = Vec::with_capacity(num_instances);
    let mut scene_points = Vec::with_capacity(num_instances);

    for row in arr.rows() {
        sdf_instances.push(SdfInstance {
            position: [row[0], row[1]],
            size: [row[2], row[3]],
            color: [row[4], row[5], row[6], row[7]],
            shape_type: row[8] as u32,
            param: row[9],
            _pad: [0.0, 0.0],
        });
        scene_points.push([row[0], row[1]]);
    }

    let title = title.to_string();

    // Release the GIL and run the event loop
    let result = py.allow_threads(move || -> Result<(), String> {
        let event_loop = EventLoop::new().map_err(|e| format!("Failed to create event loop: {e}"))?;
        let mut app = windowed::App::new(title, width, height, sdf_instances, scene_points);
        event_loop.run_app(&mut app).map_err(|e| format!("Event loop error: {e}"))?;
        app.take_error()
    });

    result.map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e))
}

#[pymodule]
fn dr2d(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<HeadlessRenderer>()?;
    m.add_class::<CoordinateMapper>()?;
    m.add_function(wrap_pyfunction!(load_parquet_columns, m)?)?;
    m.add_function(wrap_pyfunction!(show_sdf_window, m)?)?;
    Ok(())
}
