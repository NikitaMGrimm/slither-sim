//! The `slither_sim` Python extension: [`transport::Session`] in-process, with
//! typed steps and frame columns copied into `array.array` objects.
use pyo3::exceptions::{
    PyIndexError, PyOverflowError, PyRuntimeError, PyTypeError, PyUserWarning, PyValueError,
};
use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBytes, PyDict, PyString, PyTuple};
use slither_sim::transport;
use std::ffi::{CStr, CString};

pyo3::create_exception!(
    slither_sim,
    SessionEndedError,
    PyRuntimeError,
    "Advancement failed and cleared all worlds; reset the session to continue."
);

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

fn is_buffer(object: &Bound<'_, PyAny>) -> bool {
    // SAFETY: `object` is a live object, and this only inspects its type.
    unsafe { ffi::PyObject_CheckBuffer(object.as_ptr()) != 0 }
}

/// Byte order of buffer items, from a `struct` format prefix.
#[derive(Clone, Copy)]
enum Order {
    Native,
    Little,
    Big,
}

/// A buffer view of an object, released on drop. The view is boxed because
/// exporters may point its `shape` into the view itself.
struct View<'py> {
    view: Box<ffi::Py_buffer>,
    attached: Python<'py>,
}

impl<'py> View<'py> {
    fn get(object: &Bound<'py, PyAny>) -> PyResult<Self> {
        let mut view = Box::new(ffi::Py_buffer::new());
        // SAFETY: `object` is live and `view` is writable; on success the view
        // is released exactly once, by `drop`.
        if unsafe { ffi::PyObject_GetBuffer(object.as_ptr(), &mut *view, ffi::PyBUF_FULL_RO) } != 0
        {
            return Err(PyErr::fetch(object.py()));
        }
        Ok(Self {
            view,
            attached: object.py(),
        })
    }

    /// The item code and byte order of a single-item format, such as `d` or
    /// `>d`; `None` for any other format. A missing format means `B`.
    fn item(&self) -> (String, Option<(u8, Order)>) {
        let format = if self.view.format.is_null() {
            b"B".as_slice()
        } else {
            // SAFETY: exporters give a NUL-terminated format that lives as long as the view.
            unsafe { CStr::from_ptr(self.view.format) }.to_bytes()
        };
        let item = match *format {
            [code] | [b'@' | b'=', code] => Some((code, Order::Native)),
            [b'<', code] => Some((code, Order::Little)),
            [b'>' | b'!', code] => Some((code, Order::Big)),
            _ => None,
        };
        (String::from_utf8_lossy(format).into_owned(), item)
    }

    /// Copies items in C order, respecting strides without requiring alignment.
    fn bytes(&self) -> PyResult<Vec<u8>> {
        let length = usize::try_from(self.view.len).expect("buffer lengths are not negative");
        let mut out = vec![0; length];
        if length > 0 {
            // SAFETY: `out` holds exactly `view.len` bytes and the view is live.
            let status = unsafe {
                ffi::PyBuffer_ToContiguous(
                    out.as_mut_ptr().cast(),
                    &*self.view,
                    self.view.len,
                    b'C' as _,
                )
            };
            if status != 0 {
                return Err(PyErr::fetch(self.attached));
            }
        }
        Ok(out)
    }
}

impl Drop for View<'_> {
    fn drop(&mut self) {
        // SAFETY: the view was filled by `PyObject_GetBuffer`, and holding
        // `Python<'py>` keeps this thread attached.
        unsafe { ffi::PyBuffer_Release(&mut *self.view) }
    }
}

/// The bytes of a buffer whose items have one of `codes`, each `size` bytes,
/// and their byte order. Other formats raise `TypeError`.
fn read_buffer(
    object: &Bound<'_, PyAny>,
    name: &str,
    wanted: &str,
    codes: &[u8],
    size: usize,
) -> PyResult<(Vec<u8>, Order)> {
    let view = View::get(object)?;
    match view.item() {
        (_, Some((code, order)))
            if codes.contains(&code) && usize::try_from(view.view.itemsize) == Ok(size) =>
        {
            Ok((view.bytes()?, order))
        }
        (format, _) => Err(PyTypeError::new_err(format!(
            "{name} buffer must hold {wanted} items, not format {format:?}"
        ))),
    }
}

fn read_headings(object: &Bound<'_, PyAny>, out: &mut Vec<f64>) -> PyResult<()> {
    out.clear();
    if is_buffer(object) {
        let (bytes, order) = read_buffer(object, "headings", "float64", b"d", 8)?;
        out.extend(bytes.as_chunks::<8>().0.iter().map(|&item| match order {
            Order::Native => f64::from_ne_bytes(item),
            Order::Little => f64::from_le_bytes(item),
            Order::Big => f64::from_be_bytes(item),
        }));
        return Ok(());
    }
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

/// Any nonzero byte of a bool or uint8 buffer boosts.
fn read_boosts(object: &Bound<'_, PyAny>, out: &mut Vec<bool>) -> PyResult<()> {
    out.clear();
    if is_buffer(object) {
        let (bytes, _) = read_buffer(object, "boosts", "bool or uint8", b"?B", 1)?;
        out.extend(bytes.iter().map(|&byte| byte != 0));
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

// Successful scalars avoid JSON conversion. Rejections use serde's value
// decoding, like the internally tagged JSON request, to preserve its diagnostics.
fn reset_scalar<T>(value: Option<Bound<'_, PyAny>>, default: T) -> PyResult<T>
where
    for<'py> T: FromPyObject<'py, 'py> + serde::de::DeserializeOwned,
{
    let Some(value) = value else {
        return Ok(default);
    };
    if value.is_none() {
        return Ok(default);
    }
    if !value.is_instance_of::<pyo3::types::PyBool>()
        && let Ok(value) = value.extract::<T>()
    {
        return Ok(value);
    }
    let value: serde_json::Value = serde_json::from_slice(&dumps(value.py(), &value)?)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    serde_json::from_value(value).map_err(|e| PyValueError::new_err(e.to_string()))
}

fn reset_object<T: serde::de::DeserializeOwned + Default>(
    py: Python<'_>,
    value: Option<Bound<'_, PyAny>>,
) -> PyResult<T> {
    match value.filter(|v| !v.is_none()) {
        None => Ok(T::default()),
        Some(value) => serde_json::from_slice(&dumps(py, &value)?)
            .map_err(|e| PyValueError::new_err(e.to_string())),
    }
}

/// A `slither-sim-json-v3` session of independent worlds, run in this process.
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
    /// Each reply warning, about valid but unmeasured settings, is issued as a
    /// `UserWarning`.
    #[pyo3(
        signature = (seed=None, worlds=None, agents=None, decision_ticks=None, config=None, placement=None),
        text_signature = "(self, seed=1, worlds=1, agents=4, decision_ticks=6, config=None, placement=None)"
    )]
    #[allow(clippy::too_many_arguments)] // Public Python keyword signature.
    fn reset(
        &mut self,
        py: Python<'_>,
        seed: Option<Bound<'_, PyAny>>,
        worlds: Option<Bound<'_, PyAny>>,
        agents: Option<Bound<'_, PyAny>>,
        decision_ticks: Option<Bound<'_, PyAny>>,
        config: Option<Bound<'_, PyAny>>,
        placement: Option<Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let defaults = transport::ResetOptions::default();
        let options = transport::ResetOptions::from_counts(
            reset_scalar(seed, defaults.seed)?,
            reset_scalar(worlds, defaults.worlds.get())?,
            reset_scalar(agents, defaults.agents.get() as usize)?,
            reset_scalar(decision_ticks, defaults.decision_ticks.get() as usize)?,
            reset_object(py, config)?,
            reset_object(py, placement)?,
        )
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
        let prepared = py
            .detach(|| transport::Session::prepare_reset(options))
            .map_err(|mut e| {
                // Reset diagnostics predate the typed world's location metadata.
                e.world = None;
                PyValueError::new_err(e.to_string())
            })?;
        // Warnings can raise; retain both the old worlds and their frame buffers
        // until every warning has been accepted by Python.
        let category = py.get_type::<PyUserWarning>();
        for warning in &prepared.summary().warnings {
            let message = CString::new(warning.as_str()).expect("warnings have no NUL");
            PyErr::warn(py, &category, &message, 1)?;
        }
        self.inner.commit_reset(prepared);
        Ok(())
    }

    /// Steps every world like a JSON `step`. `headings` and `boosts` hold
    /// `worlds * agents` entries, world-major. A NaN heading means no action,
    /// which only dead slots may give. Raises `ValueError` on rejection and
    /// then changes no world. Raises `SessionEndedError` if advancement fails;
    /// all worlds are cleared and a reset is required to continue.
    fn step(
        &mut self,
        py: Python<'_>,
        headings: &Bound<'_, PyAny>,
        boosts: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        read_headings(headings, &mut self.headings)?;
        read_boosts(boosts, &mut self.boosts)?;
        let Self {
            inner,
            headings,
            boosts,
        } = self;
        py.detach(|| inner.step(headings, boosts))
            .map_err(|error| match &error {
                transport::StepError::Rejected(_) => PyValueError::new_err(error.to_string()),
                transport::StepError::Ended(_) => SessionEndedError::new_err(error.to_string()),
            })
    }

    /// World `world`'s current frame, copied. Indices outside the session,
    /// negative ones included, raise `IndexError`.
    fn frame(&self, py: Python<'_>, world: &Bound<'_, PyAny>) -> PyResult<Frame> {
        let world = match world.extract::<usize>() {
            Ok(world) => Some(world),
            Err(error) if error.is_instance_of::<PyOverflowError>(py) => None,
            Err(error) => return Err(error),
        };
        match world.and_then(|world| self.inner.frame(world)) {
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
        kill_score => snake_kill_score,
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
    module.add("SessionEndedError", py.get_type::<SessionEndedError>())?;
    module.add_class::<Session>()?;
    module.add_class::<Frame>()?;
    Ok(())
}
