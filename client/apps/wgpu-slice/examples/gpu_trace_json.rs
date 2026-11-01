//! CPU-only typed conversion of the pinned wgpu API trace. Stream actions so
//! offline hashing consumes actual descriptors/commands/data, not Draw records.
use serde::Deserializer as _;
use serde::de::{Error as _, SeqAccess, Visitor};
use std::io::Write;
use wgpu_core::{command::PointerReferences, device::trace::Action};

struct Actions<W>(W);
impl<'de, W: Write> Visitor<'de> for Actions<W> {
    type Value = ();
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a wgpu API trace action sequence")
    }
    fn visit_seq<A: SeqAccess<'de>>(mut self, mut actions: A) -> Result<(), A::Error> {
        while let Some(action) = actions.next_element::<Action<'de, PointerReferences>>()? {
            serde_json::to_writer(&mut self.0, &action).map_err(A::Error::custom)?;
            self.0.write_all(b"\n").map_err(A::Error::custom)?;
        }
        Ok(())
    }
}
fn run() -> Result<(), String> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("trace.ron path required")?;
    let source = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut parser = ron::Deserializer::from_str(&source).map_err(|e| e.to_string())?;
    parser
        .deserialize_seq(Actions(std::io::stdout().lock()))
        .map_err(|e| e.to_string())?;
    parser.end().map_err(|e| e.to_string())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("GPU_TRACE_CONVERSION_FAILED: {error}");
        std::process::exit(1);
    }
}

#[test]
fn pointer_json_shape_matches_the_offline_hasher() {
    use std::{marker::PhantomData, num::NonZeroUsize};
    let id = wgpu_core::id::PointerId::<wgpu_core::id::markers::Buffer>::PointerId(
        NonZeroUsize::new(123).unwrap(),
        PhantomData,
    );
    assert_eq!(
        serde_json::to_value(id).unwrap(),
        serde_json::json!({"PointerId": [123]})
    );
}
