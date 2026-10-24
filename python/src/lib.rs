//! The `slither_sim` Python extension: [`transport::Session`] in-process, with
//! typed steps and frame columns copied into `array.array` objects.
use pyo3::buffer::{Element, ElementType, PyBuffer, PyUntypedBuffer};
use pyo3::exceptions::{PyIndexError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBytes, PyDict, PyString, PyTuple};
use serde::Deserialize;
use slither_sim::transport;
use std::ffi::CStr;

static ARRAY: PyOnceLock<Py<PyAny>> = PyOnceLock::new();
static DUMPS: PyOnceLock<Py<PyAny>> = PyOnceLock::new();
static LOADS: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

/// A frame column element and its `array.array` typecode.
trait Column: Copy {
    const CODE: &'static str;
    const SIZE: usize = std::mem::size_of::<Self>();
    fn write(self, out: &mut [u8]);
}

macro_rules! column {
    ($t:ty, $code:literal) => {
        impl Column for $t {
            const CODE: &'static str = $code;
            fn write(self, out: &mut [u8]) {
                out.copy_from_slice(&self.to_ne_bytes());
            }
        }
    };
}
column!(f64, "d");
column!(u32, "I");
column!(u64, "Q");
column!(u8, "B");

/// A copy of `values` as an `array.array` of `T::CODE`.
fn array<T: Column>(py: Python<'_>, values: &[T]) -> PyResult<Py<PyAny>> {
    let bytes = PyBytes::new_with(py, values.len() * T::SIZE, |out| {
        for (out, &value) in out.chunks_exact_mut(T::SIZE).zip(values) {
            value.write(out);
        }
        Ok(())
    })?;
    let array = ARRAY.import(py, "array", "array")?;
    Ok(array.call1((T::CODE, bytes))?.unbind())
}

/// One boost flag: a `uint8` or `bool` buffer item. Any nonzero byte boosts.
#[derive(Clone, Copy, Default)]
#[repr(transparent)]
struct Flag(u8);

// SAFETY: every byte value is a valid `Flag`.
unsafe impl Element for Flag {
    fn is_compatible_format(format: &CStr) -> bool {
        matches!(
            ElementType::from_format(format),
            ElementType::UnsignedInteger { bytes: 1 } | ElementType::Bool
        )
    }
}

fn is_buffer(object: &Bound<'_, PyAny>) -> bool {
    // SAFETY: `object` is a live object, and this only inspects its type.
    unsafe { pyo3::ffi::PyObject_CheckBuffer(object.as_ptr()) != 0 }
}

/// Copies a buffer of `T` items, in C order, into `out`.
fn read_buffer<T: Element + Default>(
    py: Python<'_>,
    object: &Bound<'_, PyAny>,
    name: &str,
    wanted: &str,
    out: &mut Vec<T>,
) -> PyResult<()> {
    let buffer = PyUntypedBuffer::get(object)?;
    if !T::is_compatible_format(buffer.format()) || buffer.item_size() != size_of::<T>() {
        return Err(PyTypeError::new_err(format!(
            "{name} buffer must hold {wanted} items, not format {:?}",
            buffer.format().to_string_lossy()
        )));
    }
    let buffer: PyBuffer<T> = buffer.into_typed()?;
    out.clear();
    out.resize(buffer.item_count(), T::default());
    buffer.copy_to_slice(py, out)
}

fn read_headings(py: Python<'_>, object: &Bound<'_, PyAny>, out: &mut Vec<f64>) -> PyResult<()> {
    if is_buffer(object) {
        return read_buffer(py, object, "headings", "float64", out);
    }
    out.clear();
    for item in object.try_iter()? {
        let item = item?;
        out.push(if item.is_none() {
            f64::NAN
        } else {
            item.extract()?
        });
    }
    Ok(())
}

fn read_boosts(py: Python<'_>, object: &Bound<'_, PyAny>, out: &mut Vec<bool>) -> PyResult<()> {
    out.clear();
    if is_buffer(object) {
        let mut flags = Vec::new();
        read_buffer::<Flag>(py, object, "boosts", "bool or uint8", &mut flags)?;
        out.extend(flags.iter().map(|flag| flag.0 != 0));
        return Ok(());
    }
    for item in object.try_iter()? {
        out.push(item?.is_truthy()?);
    }
    Ok(())
}

/// `json.dumps(object, allow_nan=False)` as UTF-8.
fn dumps(py: Python<'_>, object: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    let dumps = DUMPS.import(py, "json", "dumps")?;
    let options = PyDict::new(py);
    options.set_item("allow_nan", false)?;
    let text = dumps.call((object,), Some(&options))?;
    Ok(text.cast::<PyString>()?.to_str()?.as_bytes().to_vec())
}

#[derive(Deserialize)]
struct Reply {
    error: Option<String>,
}

/// A `slither-sim-json-v2` session of independent worlds, run in this process.
///
/// Reset through keyword arguments, step with flat world-major heading and boost
/// arrays and read each world's frame as columns. Use one session from one
/// thread at a time; separate sessions run concurrently in separate threads.
#[pyclass(module = "slither_sim")]
struct Session {
    inner: transport::Session,
    headings: Vec<f64>,
    boosts: Vec<bool>,
}

#[pymethods]
impl Session {
    #[new]
    fn new() -> Self {
        Self {
            inner: transport::Session::new(),
            headings: Vec::new(),
            boosts: Vec::new(),
        }
    }

    /// Replaces the session's worlds like a JSON `reset`. Raises `ValueError`
    /// with the protocol's error text on rejection, keeping the previous worlds.
    #[pyo3(
        signature = (seed=None, worlds=None, agents=None, decision_ticks=None, config=None),
        text_signature = "(self, seed=1, worlds=1, agents=4, decision_ticks=6, config=None)"
    )]
    fn reset(
        &mut self,
        py: Python<'_>,
        seed: Option<Bound<'_, PyAny>>,
        worlds: Option<Bound<'_, PyAny>>,
        agents: Option<Bound<'_, PyAny>>,
        decision_ticks: Option<Bound<'_, PyAny>>,
        config: Option<Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        // Omitted or None members take the protocol's defaults (those of the
        // text signature); others pass through, so the protocol words every error.
        let request = PyDict::new(py);
        request.set_item("op", "reset")?;
        for (key, value) in [
            ("seed", seed),
            ("worlds", worlds),
            ("agents", agents),
            ("decisionTicks", decision_ticks),
            ("config", config),
        ] {
            if let Some(value) = value {
                request.set_item(key, value)?;
            }
        }
        let line = dumps(py, &request)?;
        let inner = &mut self.inner;
        let reply = py.detach(|| inner.request(&line));
        let reply: Reply = serde_json::from_slice(&reply).expect("replies are JSON objects");
        match reply.error {
            Some(error) => Err(PyValueError::new_err(error)),
            None => Ok(()),
        }
    }

    /// Steps every world like a JSON `step`. `headings` and `boosts` hold
    /// `worlds * agents` entries, world-major. A NaN heading means no action,
    /// which only dead slots may give. Raises `ValueError` on rejection and
    /// then changes no world.
    fn step(
        &mut self,
        py: Python<'_>,
        headings: &Bound<'_, PyAny>,
        boosts: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        read_headings(py, headings, &mut self.headings)?;
        read_boosts(py, boosts, &mut self.boosts)?;
        let Self {
            inner,
            headings,
            boosts,
        } = self;
        py.detach(|| inner.step(headings, boosts))
            .map_err(PyValueError::new_err)
    }

    /// World `world`'s current frame, copied.
    fn frame(&self, py: Python<'_>, world: usize) -> PyResult<Frame> {
        match self.inner.frame(world) {
            Some(frame) => Frame::copy(py, &frame),
            None if self.inner.worlds() == 0 => Err(PyValueError::new_err("Reset first")),
            None => Err(PyIndexError::new_err("Unknown world")),
        }
    }

    /// Answers one JSON request, given as a dict (or other JSON value), `str`
    /// or `bytes`, and returns the parsed reply object, which holds `protocol`
    /// and either `result` or `error`.
    fn request(&mut self, py: Python<'_>, message: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let line = if let Ok(bytes) = message.cast::<PyBytes>() {
            bytes.as_bytes().to_vec()
        } else if let Ok(text) = message.cast::<PyString>() {
            text.to_str()?.as_bytes().to_vec()
        } else {
            dumps(py, message)?
        };
        let inner = &mut self.inner;
        let reply = py.detach(|| inner.request(&line));
        let loads = LOADS.import(py, "json", "loads")?;
        Ok(loads.call1((PyBytes::new(py, &reply),))?.unbind())
    }

    /// Worlds of the current session; 0 before the first reset.
    #[getter]
    fn worlds(&self) -> usize {
        self.inner.worlds()
    }

    /// Agent slots per world; 0 before the first reset.
    #[getter]
    fn agents(&self) -> usize {
        self.inner.agents()
    }
}

macro_rules! frame {
    ($($table:ident { $($column:ident => $name:ident),* $(,)? })*) => {
        /// One world's frame: scalars plus `array.array` copies of its columns.
        #[pyclass(module = "slither_sim", frozen)]
        struct Frame {
            #[pyo3(get)]
            time: f64,
            #[pyo3(get)]
            ticks: u32,
            #[pyo3(get)]
            living: u32,
            #[pyo3(get)]
            radius: f64,
            #[pyo3(get)]
            vision: f64,
            $($(
                #[pyo3(get)]
                $name: Py<PyAny>,
            )*)*
        }

        impl Frame {
            fn copy(py: Python<'_>, frame: &transport::FrameRef<'_>) -> PyResult<Self> {
                Ok(Self {
                    time: frame.time,
                    ticks: frame.ticks,
                    living: frame.living,
                    radius: frame.radius,
                    vision: frame.vision,
                    $($(
                        $name: array(py, frame.$table.$column)?,
                    )*)*
                })
            }
        }
    };
}

frame! {
    snakes {
        slot => snake_slot,
        x => snake_x,
        y => snake_y,
        heading => snake_heading,
        speed => snake_speed,
        radius => snake_radius,
        length => snake_length,
        segments => snake_segments,
        fam => snake_fam,
        score => snake_score,
        kills => snake_kills,
        body_start => snake_body_start,
        body_x => snake_body_x,
        body_y => snake_body_y,
    }
    food {
        x => food_x,
        y => food_y,
        value => food_value,
        size => food_size,
        origin => food_origin,
    }
    prey {
        id => prey_id,
        x => prey_x,
        y => prey_y,
        heading => prey_heading,
        target_heading => prey_target_heading,
        speed => prey_speed,
        size => prey_size,
    }
    views {
        me => view_self,
        snakes_start => view_snakes_start,
        snakes => view_snakes,
        food_start => view_food_start,
        food => view_food,
        prey_start => view_prey_start,
        prey => view_prey,
    }
    deaths {
        time => death_time,
        victim => death_victim,
        killer => death_killer,
        reason => death_reason,
        food => death_food,
    }
}

#[pymodule]
#[pyo3(name = "slither_sim")]
fn init(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    // Columns are copied byte for byte, so each typecode must have its Rust width.
    let array = ARRAY.import(py, "array", "array")?;
    for (code, size) in [
        (f64::CODE, f64::SIZE),
        (u32::CODE, u32::SIZE),
        (u64::CODE, u64::SIZE),
        (u8::CODE, u8::SIZE),
    ] {
        let itemsize: usize = array.call1((code,))?.getattr("itemsize")?.extract()?;
        if itemsize != size {
            return Err(PyTypeError::new_err(format!(
                "array typecode {code:?} has {itemsize} bytes, not {size}"
            )));
        }
    }
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    module.add("PROTOCOL", transport::PROTOCOL)?;
    module.add("NONE", transport::NONE)?;
    module.add("NO_ORIGIN", transport::NO_ORIGIN)?;
    module.add("ORIGINS", PyTuple::new(py, transport::ORIGINS)?)?;
    module.add("REASONS", PyTuple::new(py, transport::REASONS)?)?;
    module.add_class::<Session>()?;
    module.add_class::<Frame>()?;
    Ok(())
}
