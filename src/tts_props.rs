//! Gabarits TTS partagés : silhouettes + déployables (`config/tts-props.json`).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct TtsProps {
    pub silhouettes: SilhouetteConfig,
    pub deployables: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
pub struct SilhouetteConfig {
    pub sizes: std::collections::HashMap<String, SilhouetteMesh>,
    pub camouflage: std::collections::HashMap<String, CamouflageDiffuse>,
    pub imp: std::collections::HashMap<String, ImpSilhouette>,
    pub material_index: i64,
    pub type_index: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SilhouetteMesh {
    pub mesh_url: String,
    pub collider_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CamouflageDiffuse {
    pub diffuse_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImpSilhouette {
    /// Objet TTS complet (mesh + décalcomanie) issu du sac de silhouettes.
    pub object: Value,
}

pub fn tts_props() -> &'static TtsProps {
    static PROPS: OnceLock<TtsProps> = OnceLock::new();
    PROPS.get_or_init(|| {
        serde_json::from_str(include_str!("../config/tts-props.json"))
            .expect("config/tts-props.json")
    })
}
