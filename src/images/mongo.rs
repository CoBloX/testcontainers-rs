use std::collections::HashMap;

use crate::core::Port;
use crate::{Container, Docker, Image, WaitForMessage};

const CONTAINER_IDENTIFIER: &str = "mongo";
const DEFAULT_TAG: &str = "4.0.17";

#[derive(Debug, Default, Clone)]
pub struct MongoArgs;

impl IntoIterator for MongoArgs {
    type Item = String;
    type IntoIter = ::std::vec::IntoIter<String>;

    fn into_iter(self) -> <Self as IntoIterator>::IntoIter {
        vec![].into_iter()
    }
}

#[derive(Debug)]
pub struct Mongo {
    tag: String,
    arguments: MongoArgs,
    ports: Option<Vec<Port>>,
}

impl Default for Mongo {
    fn default() -> Self {
        Mongo {
            tag: DEFAULT_TAG.to_string(),
            arguments: MongoArgs {},
            ports: None,
        }
    }
}

impl Image for Mongo {
    type Args = MongoArgs;
    type EnvVars = HashMap<String, String>;
    type Volumes = HashMap<String, String>;
    type EntryPoint = std::convert::Infallible;
    type Network = std::convert::Infallible;

    fn descriptor(&self) -> String {
        format!("{}:{}", CONTAINER_IDENTIFIER, &self.tag)
    }

    fn wait_until_ready<D: Docker>(&self, container: &Container<'_, D, Self>) {
        container
            .logs()
            .stdout
            .wait_for_message("waiting for connections on port")
            .unwrap();
    }

    fn args(&self) -> <Self as Image>::Args {
        self.arguments.clone()
    }

    fn env_vars(&self) -> Self::EnvVars {
        HashMap::new()
    }

    fn volumes(&self) -> Self::Volumes {
        HashMap::new()
    }

    fn ports(&self) -> Option<Vec<Port>> {
        self.ports.clone()
    }

    fn with_args(self, arguments: <Self as Image>::Args) -> Self {
        Mongo { arguments, ..self }
    }
}

impl Mongo {
    pub fn with_tag(self, tag_str: &str) -> Self {
        Mongo {
            tag: tag_str.to_string(),
            ..self
        }
    }

    pub fn with_mapped_port<P: Into<Port>>(mut self, port: P) -> Self {
        let mut ports = self.ports.unwrap_or_default();
        ports.push(port.into());
        self.ports = Some(ports);
        self
    }
}
