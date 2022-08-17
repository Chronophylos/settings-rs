use std::{
    env,
    fmt::Debug,
    fs::File,
    io::{BufReader, BufWriter},
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};

use log::debug;
use ron::ser::PrettyConfig;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// Error type used for all errors in this crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Emitted when the settings file could not be opened.
    #[error("Could not open settings file")]
    Open {
        source: std::io::Error,
        path: PathBuf,
    },

    /// Emitted when an error occured during deserialization.
    #[error("Could not deserialize settings file")]
    Deserialize(#[source] ron::de::SpannedError),

    /// Emitted when an error occured during serialization.
    #[error("Could not serialize settings file")]
    Serialize(#[source] ron::Error),

    /// Emitted when the settings file is not found.
    #[error("Cound not find a settings file")]
    NotFound,
}

/// A wrapper around a configuration struct.
///
/// ```rust
/// # use settings::Settings;
/// # use tempfile::tempdir;
/// # use std::{
/// #     error::Error,
/// #     fs::File,
/// #     io::Write
/// # };
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Deserialize, Serialize)]
/// struct Config {
///     pub foo: String,
///     pub bar: u32,
/// }
///
/// # fn main() -> Result<(), Box<dyn Error>> {
/// # let dir = tempdir().unwrap();
/// # let path = dir.path().join("settings.ron");
/// # let mut file = File::create(&path)?;
/// # writeln!(file, r#"Config(foo:"", bar: 0)"#)?;
/// let mut settings = Settings::<Config>::load_from(&path)?;
/// settings.foo = "Hello World".to_string();
/// settings.bar = 42;
/// settings.save()?;
///
/// let content = std::fs::read_to_string(path)?;
/// assert_eq!(content,
/// r#"Config(
///     foo: "Hello World",
///     bar: 42,
/// )"#);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings<T> {
    path: PathBuf,
    inner: T,
}

impl<T> Settings<T>
where
    T: Debug + Clone + Serialize + DeserializeOwned,
{
    /// Open the settings file given a qualifier, organization, and application name.
    ///
    /// Check multiple locations for the settings file.
    /// 1. the environment variable `{application}_CONFIG_PATH`
    /// 2. `settings.ron` in the current directory
    /// 3. `settings.ron` in the configuration directory
    ///
    /// The configuration directory depends on the operating system:
    /// ```no_run
    /// # use settings::Settings;
    /// Settings::<()>::load("com", "Foo-Corp", "Bar-App");
    /// // Linux:   /home/alice/.config/barapp
    /// // Windows: C:\Users\Alice\AppData\Roaming\Foo Corp\Bar App
    /// // macOS:   /Users/Alice/Library/Application Support/com.Foo-Corp.Bar-App
    /// ```
    pub fn load(qualifier: &str, organization: &str, application: &str) -> Result<Self, Error> {
        const FILE_NAME: &str = "settings.ron";

        let paths = [
            env::var(format!("{}_CONFIG_PATH", application.to_uppercase()))
                .ok()
                .map(PathBuf::from),
            env::current_dir().ok().map(|dir| dir.join(FILE_NAME)),
            directories::ProjectDirs::from(qualifier, organization, application)
                .map(|dir| dir.config_dir().join(FILE_NAME)),
        ];

        if let Some(path) = paths.into_iter().flatten().find(|path| path.exists()) {
            Self::load_from(path)
        } else {
            Err(Error::NotFound)
        }
    }

    /// Load the settings file from the given path.
    pub fn load_from<P>(path: P) -> Result<Self, Error>
    where
        P: AsRef<Path>,
    {
        fn inner<T>(path: &Path) -> Result<Settings<T>, Error>
        where
            T: DeserializeOwned,
        {
            debug!("Loading settings from {:?}", path);

            let file = File::open(path).map_err(|source| Error::Open {
                source,
                path: path.to_path_buf(),
            })?;
            let reader = BufReader::new(file);

            let inner: T = ron::de::from_reader(reader).map_err(Error::Deserialize)?;

            Ok(Settings {
                path: path.to_path_buf(),
                inner,
            })
        }
        inner(path.as_ref())
    }

    /// Save the settings to the last path used.
    pub fn save(&self) -> Result<(), Error> {
        self.save_to(&self.path)
    }

    /// Save the settings to the given path.
    pub fn save_to<P>(&self, path: P) -> Result<(), Error>
    where
        P: AsRef<Path>,
    {
        fn inner<T>(value: &T, path: &Path) -> Result<(), Error>
        where
            T: Serialize,
        {
            let file = File::create(&path).map_err(|source| Error::Open {
                source,
                path: path.to_path_buf(),
            })?;
            let writer = BufWriter::new(file);
            ron::ser::to_writer_pretty(writer, value, PrettyConfig::default().struct_names(true))
                .map_err(Error::Serialize)
        }
        inner(self.deref(), path.as_ref())
    }
}

impl<T> Deref for Settings<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> DerefMut for Settings<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
