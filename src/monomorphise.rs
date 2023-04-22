/// Monomorhpise the [DaemonManager]. Usually an application only needs one kind of daemon and
/// having to import it and the data object can be annoying, this macro creates a newtype that
/// behaves the same but doesn't have the generic.
///
/// ```
/// use std::sync::Arc;
/// use daemons::monomorphise;
///
/// monomorphise!(i32);
///
/// type Foo = DaemonManager; // this is actually a DaemonManager<i32>
/// let mut dm = DaemonManager::spawn(Arc::new(42));
/// ```
///
/// Also, if you have the `cron` feature enabled, this will also generate a type alias that omits
/// the data type `T` so you don't have to repeat it. See the cron module for more details.
///
/// [DaemonManager]: super::DaemonManager
#[macro_export]
macro_rules! monomorphise {
    ($data:ty) => {
        pub type DaemonManager = $crate::DaemonManager<$data>;
        $crate::_monomorphise_cron!($data);
    };
}

#[cfg(feature = "cron")]
#[doc(hidden)]
#[macro_export]
macro_rules! _monomorphise_cron {
    ($data:ty) => {
        pub type Cron<F, Fut, const H: u32, const M: u32, const S: u32> =
            $crate::cron::Cron<F, Fut, $data, H, M, S>;
    };
}

#[cfg(not(feature = "cron"))]
#[doc(hidden)]
#[macro_export]
macro_rules! _monomorphise_cron {
    ($data:ty) => {};
}
