pub mod parse;
pub mod validate;
pub mod normalize;
pub mod enrich;
pub mod batch;
pub mod emit;

use serde::{de::DeserializeOwned, Serialize};

use crate::pipeline::parse::Parseable;
use crate::pipeline::validate::Validatable;
use crate::pipeline::normalize::Normalizable;
use crate::pipeline::enrich::enrich;
use crate::pipeline::emit::emit;

/// Generic pipeline entry point. Monomorphized for each source event type S,
/// creating substantial IR when instantiated across ~20 types.
pub fn process_event<S>(raw: &str) -> Result<String, String>
where
    S: Parseable + Validatable + Normalizable + Serialize + DeserializeOwned + Clone,
{
    let parsed = S::parse(raw).map_err(|e| format!("parse error: {}", e))?;
    parsed.validate().map_err(|e| format!("validation error: {}", e))?;
    let unified = parsed.normalize().map_err(|e| format!("normalize error: {}", e))?;
    let enriched = enrich::<S>(unified);
    emit::<S>(&[enriched]).map_err(|e| format!("emit error: {}", e))
}

/// Process a batch of raw events of the same source type.
pub fn process_batch<S>(raws: &[String]) -> Result<Vec<String>, String>
where
    S: Parseable + Validatable + Normalizable + Serialize + DeserializeOwned + Clone,
{
    let mut results = Vec::new();
    for raw in raws {
        let result = process_event::<S>(raw)?;
        results.push(result);
    }
    Ok(results)
}
