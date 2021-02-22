use crate::{
    core::{env, env::Command, logs::LogStreamAsync, ports::{Ports, MapToHostPort}, WaitFor},
    Image,
};
use async_trait::async_trait;
use futures::executor::block_on;
use std::{fmt, marker::PhantomData};

/// Represents a running docker container that has been started using an async client..
///
/// Containers have a [`custom destructor`][drop_impl] that removes them as soon as they
/// go out of scope. However, async drop is not available in rust yet. This implementation
/// is using block_on. Therefore required #[tokio::test(flavor = "multi_thread")] in your test
/// to use drop effectively. Otherwise your test might stall:
///
/// ```rust
/// use testcontainers::*;
/// #[tokio::test(flavor = "multi_thread")]
/// async fn a_test() {
///     let docker = clients::Http::default();
///
///     {
///         let container = docker.run(MyImage::default()).await;
///
///         // Docker container is stopped/removed at the end of this scope.
///     }
/// }
///
/// ```
///
/// [drop_impl]: struct.ContainerAsync.html#impl-Drop
pub struct ContainerAsync<'d, I> {
    id: String,
    docker_client: Box<dyn DockerAsync>,
    image: I,
    command: Command,

    /// Tracks the lifetime of the client to make sure the container is dropped before the client.
    client_lifetime: PhantomData<&'d ()>,
}

impl<'d, I> ContainerAsync<'d, I> {
    /// Returns the id of this container.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the mapped host port for an internal port of this docker container.
    ///
    /// This method does **not** magically expose the given port, it simply performs a mapping on
    /// the already exposed ports. If a docker container does not expose a port, this method will panic.
    ///
    /// # Panics
    ///
    /// This method panics if the given port is not mapped.
    /// Testcontainers is designed to be used in tests only. If a certain port is not mapped, the container
    /// is unlikely to be useful.
    pub async fn get_host_port<T>(&self, internal_port: T) -> T
    where
        T: fmt::Debug,
        Ports: MapToHostPort<T>
    {
        self.docker_client
            .ports(&self.id)
            .await
            .map_to_host_port(&internal_port)
            .unwrap_or_else(|| {
                panic!(
                    "container {:?} does not expose port {:?}",
                    self.id, internal_port
                )
            })
    }

    pub async fn start(&self) {
        self.docker_client.start(&self.id).await
    }

    pub async fn stop(&self) {
        log::debug!("Stopping docker container {}", self.id);

        self.docker_client.stop(&self.id).await
    }

    pub async fn rm(self) {
        log::debug!("Deleting docker container {}", self.id);

        self.docker_client.rm(&self.id).await
    }

    async fn drop_async(&self) {
        match self.command {
            env::Command::Remove => self.docker_client.rm(&self.id).await,
            env::Command::Keep => {}
        }
    }
}

impl<'d, I> fmt::Debug for ContainerAsync<'d, I>
where
    I: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContainerAsync")
            .field("id", &self.id)
            .field("image", &self.image)
            .finish()
    }
}

/// Represents Docker operations as an async trait.
///
/// This trait is `pub(crate)` to make sure we can make changes to this API without breaking clients.
/// Users should interact through the [`ContainerAsync`] API.
#[async_trait]
pub(crate) trait DockerAsync
where
    Self: Sync,
{
    fn stdout_logs<'s>(&'s self, id: &str) -> LogStreamAsync<'s>;
    fn stderr_logs<'s>(&'s self, id: &str) -> LogStreamAsync<'s>;
    async fn ports(&self, id: &str) -> Ports;
    async fn rm(&self, id: &str);
    async fn stop(&self, id: &str);
    async fn start(&self, id: &str);
}

impl<'d, I> ContainerAsync<'d, I>
where
    I: Image,
{
    /// Constructs a new container given an id, a docker client and the image.
    /// ContainerAsync::new().await
    pub(crate) async fn new(
        id: String,
        docker_client: impl DockerAsync + 'static,
        image: I,
        command: env::Command,
    ) -> ContainerAsync<'d, I> {
        let container = ContainerAsync {
            id,
            docker_client: Box::new(docker_client),
            image,
            command,
            client_lifetime: PhantomData,
        };

        container.block_until_ready().await;

        container
    }

    async fn block_until_ready(&self) {
        log::debug!("Waiting for container {} to be ready", self.id);

        for condition in self.image.ready_conditions() {
            match condition {
                WaitFor::StdOutMessage { message } => self
                    .docker_client
                    .stdout_logs(&self.id)
                    .wait_for_message(&message)
                    .await
                    .unwrap(),
                WaitFor::StdErrMessage { message } => self
                    .docker_client
                    .stderr_logs(&self.id)
                    .wait_for_message(&message)
                    .await
                    .unwrap(),
                WaitFor::Duration { length } => {
                    tokio::time::sleep(length).await;
                }
                WaitFor::Nothing => {}
            }
        }

        log::debug!("Container {} is now ready!", self.id);
    }
}

impl<'d, I> Drop for ContainerAsync<'d, I> {
    fn drop(&mut self) {
        block_on(self.drop_async())
    }
}
