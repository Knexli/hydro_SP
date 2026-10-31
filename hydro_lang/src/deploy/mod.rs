//! Infrastructure for deploying Hydro programs to the cloud using [`hydro_deploy`].

#[cfg(any(feature = "deploy", feature = "sim", feature = "deploy_integration"))]
pub(crate) mod deploy_runtime;

#[cfg(any(feature = "docker_runtime", feature = "ecs_runtime", feature = "knative_runtime"))]
pub mod deploy_runtime_containerized;

#[cfg(feature = "ecs_runtime")]
pub mod deploy_runtime_containerized_ecs;

#[cfg(feature = "knative_runtime")]
pub mod deploy_runtime_containerized_knative;

#[cfg(any(feature = "maelstrom", feature = "maelstrom_runtime"))]
pub mod maelstrom;

#[cfg(stageleft_runtime)]
#[cfg(feature = "deploy")]
#[cfg_attr(docsrs, doc(cfg(feature = "deploy")))]
pub use crate::compile::init_test;

#[cfg(stageleft_runtime)]
#[cfg(feature = "deploy")]
#[cfg_attr(docsrs, doc(cfg(feature = "deploy")))]
pub mod deploy_graph;

#[cfg(stageleft_runtime)]
#[cfg(feature = "deploy")]
#[cfg_attr(docsrs, doc(cfg(feature = "deploy")))]
pub use deploy_graph::*;

#[cfg(stageleft_runtime)]
#[cfg(any(feature = "docker_deploy", feature = "knative_deploy"))]
#[cfg_attr(docsrs, doc(cfg(feature = "docker_deploy")))]
pub mod deploy_graph_containerized;

#[cfg(stageleft_runtime)]
#[cfg(any(feature = "docker_deploy", feature = "knative_deploy"))]
#[cfg_attr(docsrs, doc(cfg(feature = "docker_deploy")))]
pub use deploy_graph_containerized::*;

#[cfg(stageleft_runtime)]
#[cfg(feature = "ecs_deploy")]
#[cfg_attr(docsrs, doc(cfg(feature = "ecs_deploy")))]
pub mod deploy_graph_containerized_ecs;

#[cfg(stageleft_runtime)]
#[cfg(feature = "ecs_deploy")]
#[cfg_attr(docsrs, doc(cfg(feature = "ecs_deploy")))]
pub use deploy_graph_containerized_ecs::*;

#[cfg(stageleft_runtime)]
#[cfg(feature = "knative_deploy")]
pub mod deploy_graph_containerized_knative;
#[cfg(stageleft_runtime)]
#[cfg(feature = "knative_deploy")]
pub use deploy_graph_containerized_knative::*;
