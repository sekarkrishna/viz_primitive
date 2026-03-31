mod windowed;

use numpy::{IntoPyArray, PyArray1, PyArray3, PyReadonlyArray1, PyReadonlyArray2};
use pyo3::prelude::*;
use ndarray::Array3;
use std::collections::HashMap;
use std::path::Path;

use ::dr2d::headless::HeadlessRenderer as RustHeadlessRenderer;
use ::dr2d::renderer::sdf_pipeline::SdfInstance;
use ::dr2d::data::parquet_loader::{ParquetLoader, ParquetError};
use ::dr2d::data::coord_mapper::{
    CoordinateMapper as RustCoordinateMapper,
    DataRange,
};

use winit::event_loop::EventLoop;
use winit::platform::pump_events::EventLoopExtPumpEvents;

use windowed::{ZoomMode, SlideState, StoryboardApp};

use std::cell::RefCell;

thread_local! {
    static EVENT_LOOP: RefCell<EventLoop<()>> = RefCell::new(
        EventLoop::new().expect("Failed to create initial event loop")
    );
}

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
/// layer_sizes: optional list of integers specifying the instance count per layer.
///   If provided, sum must equal the number of rows in instances.
///   Enables digit-key (1–9) layer toggle in the interactive window.
///
/// Blocks until the window is closed. Releases the GIL while the event loop runs.
#[pyfunction]
#[pyo3(signature = (instances, width, height, title="dr2d", layer_sizes=None))]
fn show_sdf_window(
    py: Python<'_>,
    instances: PyReadonlyArray2<f32>,
    width: u32,
    height: u32,
    title: &str,
    layer_sizes: Option<Vec<usize>>,
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

    // Validate layer_sizes if provided
    let sizes = match layer_sizes {
        Some(sizes) => {
            let sum: usize = sizes.iter().sum();
            if sum != num_instances {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "layer_sizes sum ({sum}) does not equal instance count ({num_instances})"
                )));
            }
            sizes
        }
        None => Vec::new(),
    };

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
        let mut app = windowed::App::new(title, width, height, sdf_instances, scene_points, sizes);
        EVENT_LOOP.with(|el| {
            let mut event_loop = el.borrow_mut();
            loop {
                let timeout = std::time::Duration::from_millis(16); // ~60fps polling
                let status = event_loop.pump_app_events(Some(timeout), &mut app);
                if let winit::platform::pump_events::PumpStatus::Exit(_) = status {
                    break;
                }
                if app.should_exit() {
                    break;
                }
            }
        });
        app.take_error()
    });

    result.map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e))
}

/// Open an interactive storyboard window with slide navigation.
///
/// instances: Nx10 float32 array with columns:
///   [position_x, position_y, size_x, size_y, r, g, b, a, shape_type, param]
///
/// layer_sizes: list of integers specifying the instance count per layer.
///   Sum must equal the number of rows in instances.
///
/// slides: list of dicts, each with keys:
///   "zoom_mode" (str: "fit" or "explicit"),
///   "pan_x" (float), "pan_y" (float), "zoom" (float),
///   "visible_layers" (list of 0-based ints; empty = all),
///   "title" (str).
///
/// Blocks until the window is closed. Releases the GIL while the event loop runs.
#[pyfunction]
#[pyo3(signature = (instances, width, height, layer_sizes, slides))]
fn show_storyboard_window(
    py: Python<'_>,
    instances: PyReadonlyArray2<f32>,
    width: u32,
    height: u32,
    layer_sizes: Vec<usize>,
    slides: Vec<HashMap<String, PyObject>>,
) -> PyResult<()> {
    // Validate dimensions
    if width == 0 || height == 0 {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "width and height must be > 0",
        ));
    }

    let arr = instances.as_array();
    let shape = arr.shape();

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

    // Validate layer_sizes
    let sum: usize = layer_sizes.iter().sum();
    if sum != num_instances {
        return Err(pyo3::exceptions::PyValueError::new_err(format!(
            "layer_sizes sum ({sum}) does not equal instance count ({num_instances})"
        )));
    }

    // Parse slide dicts into SlideState
    let mut parsed_slides = Vec::with_capacity(slides.len());
    for (i, slide_dict) in slides.iter().enumerate() {
        let zoom_mode_str: String = slide_dict
            .get("zoom_mode")
            .ok_or_else(|| {
                pyo3::exceptions::PyValueError::new_err(format!(
                    "Slide {i}: missing 'zoom_mode' key"
                ))
            })?
            .extract(py)?;

        let zoom_mode = match zoom_mode_str.as_str() {
            "fit" => ZoomMode::Fit,
            "explicit" => ZoomMode::Explicit,
            other => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "Slide {i}: zoom_mode must be 'fit' or 'explicit', got '{other}'"
                )));
            }
        };

        let pan_x: f32 = slide_dict
            .get("pan_x")
            .map(|v| v.extract(py))
            .transpose()?
            .unwrap_or(0.0);

        let pan_y: f32 = slide_dict
            .get("pan_y")
            .map(|v| v.extract(py))
            .transpose()?
            .unwrap_or(0.0);

        let zoom: f32 = slide_dict
            .get("zoom")
            .map(|v| v.extract(py))
            .transpose()?
            .unwrap_or(1.0);

        let visible_layers: Vec<usize> = slide_dict
            .get("visible_layers")
            .map(|v| v.extract(py))
            .transpose()?
            .unwrap_or_default();

        let title: String = slide_dict
            .get("title")
            .map(|v| v.extract(py))
            .transpose()?
            .unwrap_or_default();

        parsed_slides.push(SlideState {
            zoom_mode,
            pan_x,
            pan_y,
            zoom,
            visible_layers,
            title,
        });
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

    // Release the GIL and run the event loop
    let result = py.allow_threads(move || -> Result<(), String> {
        let mut app = StoryboardApp::new(
            width,
            height,
            sdf_instances,
            scene_points,
            layer_sizes,
            parsed_slides,
        );
        EVENT_LOOP.with(|el| {
            let mut event_loop = el.borrow_mut();
            loop {
                let timeout = std::time::Duration::from_millis(16);
                let status = event_loop.pump_app_events(Some(timeout), &mut app);
                if let winit::platform::pump_events::PumpStatus::Exit(_) = status {
                    break;
                }
                if app.should_exit() {
                    break;
                }
            }
        });
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
    m.add_function(wrap_pyfunction!(show_storyboard_window, m)?)?;
    Ok(())
}
