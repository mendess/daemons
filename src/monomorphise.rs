/// Monomorhpise the [DaemonManager]. Usually an application only needs one kind of daemon and
/// having to import it and the data object can be annoying, this macro creates a newtype that
/// behaves the same but doens't have the generic.
///
/// ```
/// use std::sync::Arc;
/// use daemons::monomorphise;
///
/// monomorphise!(i32);
///
/// type Foo = DaemonManager; // this is actually a DaemonManager<i32>
/// let mut dm = DaemonManager::new(Arc::new(42));
/// ```
/// Another advantage of this is the ability to implement external traits on [DaemonManager]s that
/// are generic over a type that also doesn't belong in your crate.
///
/// ```compile_fail
/// use std::sync::Arc;
/// use daemons::DaemonManager;
///
/// impl Default for DaemonManager<String> {
///     fn default() -> Self {
///         DaemonManager::new(Arc::new("default-string".into()))
///     }
/// }
/// ```
///
/// ```
/// use std::sync::Arc;
/// use daemons::monomorphise;
///
/// monomorphise!(String);
///
/// impl Default for DaemonManager {
///     fn default() -> Self {
///         DaemonManager::new(Arc::new("default-string".into()))
///     }
/// }
/// ```
///
/// [DaemonManager]: super::DaemonManager
#[macro_export]
macro_rules! monomorphise {
    ($data:ty) => {
        pub struct DaemonManager($crate::DaemonManager<$data>);

        impl DaemonManager {
            #[allow(dead_code)]
            pub fn new(data: ::std::sync::Arc<$data>) -> Self {
                data.into()
            }
        }

        impl From<::std::sync::Arc<$data>> for DaemonManager {
            fn from(data: ::std::sync::Arc<$data>) -> Self {
                Self(data.into())
            }
        }

        impl ::std::ops::Deref for DaemonManager {
            type Target = $crate::DaemonManager<$data>;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl ::std::ops::DerefMut for DaemonManager {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }
    };
}
