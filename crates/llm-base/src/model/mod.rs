//! Large language model traits and types

use std::{
    collections::HashMap,
    error::Error,
    fmt::Debug,
    io::{BufRead, Write},
    path::{Path, PathBuf},
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;

use crate::{
    loader::TensorLoader, vocabulary::TokenId, FileType, InferenceParameters, InferenceSession,
    InferenceSessionConfig, LoadError, LoadProgress, Vocabulary,
};

/// Common functions for model evaluation
pub mod common;

macro_rules! define_model_dynamic_override_value {
    ($(($name:ident, $type:ty, $doc:literal)),*) => {
        #[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
        #[serde(untagged)]
        /// Valid value types for dynamic model overrides.
        pub enum ModelDynamicOverrideValue {
            $(#[doc=$doc] $name($type),)*
        }

        $(
            impl TryFrom<ModelDynamicOverrideValue> for $type {
                type Error = ();

                fn try_from(value: ModelDynamicOverrideValue) -> Result<Self, Self::Error> {
                    match value {
                        ModelDynamicOverrideValue::$name(value) => Ok(value),
                        _ => Err(()),
                    }
                }
            }

            impl From<$type> for ModelDynamicOverrideValue {
                fn from(value: $type) -> Self {
                    Self::$name(value)
                }
            }
        )*
    };
}

define_model_dynamic_override_value!(
    (Bool, bool, "A boolean value"),
    (String, String, "A string value"),
    (Int, i64, "An integer value"),
    (Float, f64, "A float value")
);

/// Model options that can be overridden by the user at runtime.
///
/// Each model has its own set of options that can be overridden.
/// However, the calling code may not know the type of the model
/// at compile time. This type is used to store the overrides
/// for a model in a generic way.
#[derive(Debug, PartialEq, Serialize, Deserialize, Default, Clone)]
#[serde(transparent)]
pub struct ModelDynamicOverrides(pub HashMap<String, ModelDynamicOverrideValue>);
impl ModelDynamicOverrides {
    /// Get the value of the override with the given `key`.
    pub fn get<T: TryFrom<ModelDynamicOverrideValue>>(&self, key: &str) -> Option<T> {
        self.0
            .get(key)
            .cloned()
            .and_then(|value| T::try_from(value).ok())
    }

    /// Merge the overrides from `other` into this one.
    pub fn merge(&mut self, other: impl Into<Self>) -> &mut Self {
        self.0.extend(other.into().0.into_iter());
        self
    }

    /// Insert a new override with the given `key` and `value`.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<ModelDynamicOverrideValue>) {
        self.0.insert(key.into(), value.into());
    }
}
impl From<ModelDynamicOverrides> for () {
    fn from(_: ModelDynamicOverrides) -> Self {}
}
impl From<()> for ModelDynamicOverrides {
    fn from(_: ()) -> Self {
        Self::default()
    }
}

/// Interfaces for creating and interacting with a large language model with a known type
/// of [hyperparameters](https://en.wikipedia.org/wiki/Hyperparameter_(machine_learning)).
pub trait KnownModel: Send + Sync {
    /// Hyperparameters for the model.
    type Hyperparameters: Hyperparameters;

    /// Model options that can be overridden by the user.
    ///
    /// If there are no options to override, use `()`.
    type Overrides: Serialize
        + DeserializeOwned
        + Default
        + From<ModelDynamicOverrides>
        + Into<ModelDynamicOverrides>;

    /// Load this model from the `path` and configure it per the `params`. The status
    /// of the loading process will be reported through `load_progress_callback`. This
    /// is a helper function on top of [llm_base::load](crate::load).
    fn load(
        path: &Path,
        params: ModelParameters,
        overrides: Option<Self::Overrides>,
        load_progress_callback: impl FnMut(LoadProgress),
    ) -> Result<Self, LoadError>
    where
        Self: Sized,
    {
        crate::load(path, params, overrides, load_progress_callback)
    }

    /// Creates a new model from the provided [ModelParameters] hyperparameters.
    /// This function is called by the [load](crate::loader::load) function.
    fn new<E: Error>(
        hyperparameters: Self::Hyperparameters,
        params: ModelParameters,
        overrides: Option<Self::Overrides>,
        vocabulary: Vocabulary,
        tensor_loader: impl TensorLoader<E>,
    ) -> Result<Self, E>
    where
        Self: Sized;

    /// Starts a new `InferenceSession` for this model.
    fn start_session(&self, config: InferenceSessionConfig) -> InferenceSession;

    /// This function is called by the provided [InferenceSession]; it will use this model
    /// and the [InferenceParameters] to generate output by evaluating the `input_tokens`.
    /// The [OutputRequest] is used to specify additional data to fetch from the
    /// model.
    fn evaluate(
        &self,
        session: &mut InferenceSession,
        params: &InferenceParameters,
        input_tokens: &[TokenId],
        output_request: &mut OutputRequest,
    );

    /// Get the vocabulary (loaded from the GGML file) for this model.
    fn vocabulary(&self) -> &Vocabulary;

    /// Get the context size (configured with [ModelParameters::context_size]) used by
    /// this model.
    fn context_size(&self) -> usize;

    /// Get the beginning of text/beginning of string token ID, if available. This value is defined by model implementers.
    fn bot_token_id(&self) -> Option<TokenId>;

    /// Get the end of text/end of string token ID. This value is defined by model implementers.
    fn eot_token_id(&self) -> TokenId;

    /// Get the default [InferenceParameters] for this model (used by
    /// [InferenceSession::infer]). This value is configured through
    /// [ModelParameters::inference_parameters].
    fn inference_parameters(&self) -> &InferenceParameters;
}

/// A type-erased model to allow for interacting with a model without knowing
/// its hyperparameters.
pub trait Model: Send + Sync {
    /// Starts a new `InferenceSession` for this model.
    fn start_session(&self, config: InferenceSessionConfig) -> InferenceSession;

    /// This function is called by the provided [InferenceSession]; it will use this model
    /// and the [InferenceParameters] to generate output by evaluating the `input_tokens`.
    /// The [OutputRequest] is used to specify additional data to fetch from the
    /// model.
    fn evaluate(
        &self,
        session: &mut InferenceSession,
        params: &InferenceParameters,
        input_tokens: &[TokenId],
        output_request: &mut OutputRequest,
    );

    /// Get the vocabulary (loaded from the GGML file) for this model.
    fn vocabulary(&self) -> &Vocabulary;

    /// Get the context size (configured with [ModelParameters::context_size]) used by
    /// this model.
    fn context_size(&self) -> usize;

    /// Get the beginning of text/beginning of string token ID, if available. This value is defined by model implementers.
    fn bot_token_id(&self) -> Option<TokenId>;

    /// Get the end of text/end of string token ID. This value is defined by model implementers.
    fn eot_token_id(&self) -> TokenId;

    /// Get the default [InferenceParameters] for this model (used by
    /// [InferenceSession::infer]). This value is configured through
    /// [ModelParameters::inference_parameters].
    fn inference_parameters(&self) -> &InferenceParameters;

    /// Clone this model into a boxed trait object.
    fn clone_box(&self) -> Box<dyn Model>;
}

impl Clone for Box<dyn Model> {
    fn clone(&self) -> Box<dyn Model> {
        self.clone_box()
    }
}

impl<H: Hyperparameters, M: KnownModel<Hyperparameters = H> + Clone + 'static> Model for M {
    fn start_session(&self, config: InferenceSessionConfig) -> InferenceSession {
        KnownModel::start_session(self, config)
    }

    fn evaluate(
        &self,
        session: &mut InferenceSession,
        params: &InferenceParameters,
        input_tokens: &[TokenId],
        output_request: &mut OutputRequest,
    ) {
        KnownModel::evaluate(self, session, params, input_tokens, output_request)
    }

    fn vocabulary(&self) -> &Vocabulary {
        KnownModel::vocabulary(self)
    }

    fn context_size(&self) -> usize {
        KnownModel::context_size(self)
    }

    fn bot_token_id(&self) -> Option<TokenId> {
        KnownModel::bot_token_id(self)
    }

    fn eot_token_id(&self) -> TokenId {
        KnownModel::eot_token_id(self)
    }

    fn inference_parameters(&self) -> &InferenceParameters {
        KnownModel::inference_parameters(self)
    }

    fn clone_box(&self) -> Box<dyn Model> {
        Box::new(self.clone())
    }
}

/// Implemented by model hyperparameters for interacting with hyperparameters
/// without knowing what they are, as well as writing/reading them as required.
pub trait Hyperparameters: Sized + Default + Debug {
    /// Read the parameters in GGML format from a reader.
    fn read_ggml(reader: &mut dyn BufRead) -> Result<Self, LoadError>;

    /// Write the parameters in GGML format to a writer.
    fn write_ggml(&self, writer: &mut dyn Write) -> Result<(), HyperparametersWriteError>;

    /// Get the number of tokens in the vocabulary.
    fn n_vocabulary(&self) -> usize;

    /// Get the filetype of the model.
    fn file_type(&self) -> Option<FileType>;

    /// Get mutable access to filetype of the model.
    fn file_type_mut(&mut self) -> Option<&mut FileType>;
}
#[derive(Error, Debug)]
/// Reported from functions that write
pub enum HyperparametersWriteError {
    #[error("non-specific I/O error")]
    /// A non-specific IO error.
    Io(#[from] std::io::Error),
    #[error("invalid integer conversion")]
    /// One of the integers encountered could not be converted to a more appropriate type.
    InvalidIntegerConversion(#[from] std::num::TryFromIntError),
}

/// Parameters for tuning model instances
pub struct ModelParameters {
    /// For [GGML formats](ggml::ContainerType) that support it, [mmap](https://en.wikipedia.org/wiki/Mmap)
    /// is the default. Although mmap typically improves performance, setting this value to `false` may
    /// be preferred in resource-constrained environments.
    pub prefer_mmap: bool,
    /// The context size ("memory") the model should use when evaluating a prompt. A larger context
    /// consumes more resources, but produces more consistent and coherent responses.
    pub context_size: usize,
    /// Default InferenceParameters to use when [evaluating](Model::evaluate) a prompt with this model.
    pub inference_parameters: InferenceParameters,
    /// The [LoRA](https://arxiv.org/abs/2106.09685) adapters to use when loading the model. If `None`, no adapters will be used.
    pub lora_adapters: Option<Vec<PathBuf>>,
}

impl Default for ModelParameters {
    fn default() -> Self {
        Self {
            prefer_mmap: true,
            context_size: 2048,
            inference_parameters: Default::default(),
            lora_adapters: None,
        }
    }
}

/// Used in a call to [Model::evaluate] or [InferenceSession::infer] to request
/// information from the model. If a value is set to `Some`, the `Vec` will be
/// cleared, resized, and filled with the related data.
#[derive(Default, Debug, PartialEq, Clone)]
pub struct OutputRequest {
    /// Returns all the logits for evaluation. A logit represents the likelihood
    /// that a given token will be generated based on the tokens that have been
    /// evaluated or generated so far. Output shape is `n_batch * n_vocab`.
    pub all_logits: Option<Vec<f32>>,
    /// Returns all the embeddings for an evaluation. An embedding is a vector
    /// that measures the relatedness of text strings. Output shape is
    /// `n_batch * n_embd`.
    pub embeddings: Option<Vec<f32>>,
}
