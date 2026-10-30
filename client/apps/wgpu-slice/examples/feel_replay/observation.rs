//! Decode the v3 flat recorder ABI, matching observation.mjs.
use serde_json::{Value, json};
use slither_client::feel_trace::Observation;

pub fn decode(row: &Observation, geometry: &Value) -> Result<Value, String> {
    let mut cells = row.values.iter().copied();
    let mut n = || {
        cells
            .next()
            .ok_or_else(|| "Truncated core observation".to_string())
    };
    fn point(n: &mut impl FnMut() -> Result<f64, String>) -> Result<Value, String> {
        Ok(json!({"x":n()?, "y":n()?}))
    }
    let css = |p: &Value| {
        json!({
            "x": p["x"].as_f64().unwrap() * geometry["css"]["width"].as_f64().unwrap() / geometry["backing"]["width"].as_f64().unwrap(),
            "y": p["y"].as_f64().unwrap() * geometry["css"]["height"].as_f64().unwrap() / geometry["backing"]["height"].as_f64().unwrap()
        })
    };
    let delta = json!({"raw":n()?, "lagged":n()?, "steps":n()?, "doubledSteps":n()?});
    let multiplier = n()?;
    let mut camera =
        json!({"x":n()?, "y":n()?, "offset":point(&mut n)?, "follow":n()? != 0., "zoom":n()?});
    camera["targetZoom"] = camera["zoom"].clone();
    let mut own = Value::Null;
    if let Some(token) = row.token {
        own = json!({"token":token.to_string(), "expired":n()? != 0., "deathProgress":n()?, "spawnProgress":n()?, "opacity":n()?,
            "rawHead":point(&mut n)?, "correctedHead":point(&mut n)?, "rawHeading":n()?,
            "correctedHeading":n()?, "wantedHeading":n()?, "gaze":n()?,
            "pupils":{"left":point(&mut n)?}});
        own["pupils"]["right"] = own["pupils"]["left"].clone();
        own["offsets"] = json!({"x":n()?, "y":n()?, "fraction":n()?, "heading":n()?});
        own["speed"] = json!(n()?);
        own["visualSpeed"] = json!(n()?);
        own["boost"] = json!(n()? != 0.);
        own["boostPhase"] = json!(n()?);
        own["size"] = json!({"segments":n()?, "fraction":n()?, "length":n()?});
        camera["targetZoom"] = json!(n()?);
        let count = n()?;
        if count.fract() != 0. || !(0. ..=8193.).contains(&count) {
            return Err("Invalid body count".into());
        }
        own["body"] = (0..count as usize)
            .map(|_| point(&mut n).map(|p| css(&p)))
            .collect::<Result<Value, _>>()?;
        own["projected"] = Value::Null;
        if own["expired"] == false {
            let head = point(&mut n)?;
            let pupils = json!([point(&mut n)?, point(&mut n)?]);
            own["projected"] = json!({"backing":{"head":head, "pupils":pupils},
                "css":{"head":css(&head), "pupils":[css(&pupils[0]),css(&pupils[1])]}});
        }
    }
    if cells.next().is_some() {
        return Err("Unknown core observation cells".into());
    }
    Ok(json!({"delta":delta, "multiplier":multiplier, "camera":camera, "own":own}))
}
