// Appended only to the archived checkpoint module. Validate record shapes one
// at a time; zero-sized vectors retain counts without retaining JSON trees.
// The original typed decoder and every state/configuration validation still run.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StudyEnvelopeShape {
    format: Value,
    rules_revision: Value,
    producer: Value,
    config: Value,
    state: StudyObject<StudyStateShape>,
}

#[derive(Serialize)]
struct StudyObject<T>(T);
impl<'de, T: Deserialize<'de>> Deserialize<'de> for StudyObject<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor<T>(std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for ObjectVisitor<T> {
            type Value = T;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("checkpoint object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, map: A) -> Result<T, A::Error> {
                T::deserialize(serde::de::value::MapAccessDeserializer::new(map))
            }
        }
        deserializer.deserialize_map(ObjectVisitor(std::marker::PhantomData)).map(Self)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StudyStateShape {
    time: Value,
    radius: Value,
    radius_update_index: Value,
    next_join: Value,
    random: Value,
    prey_random: Value,
    next_prey_id: Value,
    ambient_clock: Value,
    prey_clock: Value,
    prey_motion_clock: Value,
    min_mass: Value,
    failure: Value,
    #[serde(serialize_with = "study_empty_array")]
    slots: Vec<StudySlotShape>,
    #[serde(serialize_with = "study_empty_array")]
    food: Vec<StudyFoodShape>,
    #[serde(serialize_with = "study_empty_array")]
    prey: Vec<StudyPreyShape>,
    #[serde(serialize_with = "study_empty_array")]
    deaths: Vec<StudyDeathShape>,
}

fn study_empty_array<S: serde::Serializer, T>(_: &[T], serializer: S) -> Result<S::Ok, S::Error> {
    Vec::<Value>::new().serialize(serializer)
}

macro_rules! study_record_shape {
    ($name:ident, $keys:expr) => {
        #[derive(Serialize)]
        struct $name;
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = Value::deserialize(deserializer)?;
                fields(&value, $keys, "study.record").map_err(serde::de::Error::custom)?;
                Ok(Self)
            }
        }
    };
}

study_record_shape!(StudyFoodShape, &["x", "y", "value", "size", "origin"]);
study_record_shape!(StudyPreyShape, &[
    "id", "position", "heading", "targetHeading", "speed", "size", "nextDecision", "mode"
]);
study_record_shape!(StudyDeathShape, &["time", "victim", "killer", "reason", "food"]);

#[derive(Serialize)]
struct StudySlotShape;
impl<'de> Deserialize<'de> for StudySlotShape {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let slot = Value::deserialize(deserializer)?;
        fields(&slot, &[
            "snake", "boost", "speed", "kiosk", "pitEntered", "deathTime", "join", "killTimes"
        ], "study.slot").map_err(serde::de::Error::custom)?;
        for (name, keys) in [
            ("snake", &[
                "id", "head", "body", "heading", "speed", "radius", "length", "segments",
                "fam", "score", "kills", "alive"
            ][..]),
            ("boost", &[
                "held", "active", "exhausted", "clock", "activeBoostSeconds", "boostMassSpent"
            ]),
            ("speed", &["speed", "progress"]),
            ("kiosk", &["anchor", "heldSeconds", "scoreCredit"]),
        ] {
            fields(&slot[name], keys, "study.slot.record").map_err(serde::de::Error::custom)?;
        }
        Ok(Self)
    }
}

fn study_checkpoint_shape(bytes: &[u8]) -> Result<Value, SimError> {
    let shape: StudyObject<StudyEnvelopeShape> = decode(bytes)?;
    // Arrays were shape-checked above. The original state_schema checks the
    // small header/configuration/state fields in this surrogate; the original
    // Envelope decoder then reads all records from the unchanged input bytes.
    serde_json::to_value(shape).map_err(|error| {
        invalid(Constraint::InconsistentState, "study.shape", error.to_string())
    })
}

impl Simulator {
    pub fn study_checkpoint_checks() -> Value {
        let mut config = Config::sandbox();
        config.food_count = 1;
        config.prey.count = 1;
        config.ambient_replenish_per_second = 0.;
        let world = Self::new(crate::WorldOptions {
            seed: 7, config,
            population: crate::InitialPopulation::Dormant { ids: vec!["study".into()] },
        }).unwrap();
        let bytes = serde_json::to_vec(&world.checkpoint().unwrap()).unwrap();
        let original = Self::from_checkpoint(&bytes, RestoreOptions::default()).unwrap();
        let bounded = Self::study_from_checkpoint(&bytes, RestoreOptions::default()).unwrap();
        assert_eq!(serde_json::to_vec(&original.checkpoint().unwrap()).unwrap(), bytes);
        assert_eq!(serde_json::to_vec(&bounded.checkpoint().unwrap()).unwrap(), bytes);
        let raw: Value = serde_json::from_slice(&bytes).unwrap();
        let reject = |bad: Value| {
            let encoded = serde_json::to_vec(&bad).unwrap();
            assert!(Self::from_checkpoint(&encoded, RestoreOptions::default()).is_err());
            assert!(study_checkpoint_shape(&encoded).and_then(|v| state_schema(&v)).is_err());
        };
        let mut bad = raw.clone();
        bad["state"]["food"][0].as_object_mut().unwrap().remove("size");
        reject(bad);
        let mut bad = raw.clone();
        bad["state"]["food"][0]["unloaded"] = Value::Bool(true);
        reject(bad);
        let mut bad = raw.clone();
        bad["state"]["slots"][0]["snake"].as_object_mut().unwrap().remove("heading");
        reject(bad);
        let mut bad = raw.clone();
        bad["state"]["slots"][0]["speed"] = serde_json::json!([0, 0]);
        reject(bad);
        let mut bad = raw.clone();
        bad["state"]["prey"][0].as_object_mut().unwrap().remove("nextDecision");
        reject(bad);
        let mut bad = raw.clone();
        bad["state"]["deaths"] = serde_json::json!([{"time":0,"victim":"study","killer":null,"reason":"body"}]);
        reject(bad);
        let mut bad = raw.clone();
        bad["state"].as_object_mut().unwrap().remove("random");
        reject(bad);
        let mut bad = raw;
        bad["state"] = serde_json::json!([]);
        reject(bad);
        serde_json::json!({"validRoundTrip":true,"originalImporterCompared":true,"rejectedShapes":8})
    }
}
