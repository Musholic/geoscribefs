// Helper macro to forward and trace calls to the inner PassthroughFs
macro_rules! forward_to_inner {
    () => {};
    (
        #[skip($($skip:ident),* $(,)?)]
        fn $name:ident(&self $(, $arg:ident : $arg_ty:ty )*) $(-> $ret:ty)?;
        $($rest:tt)*
    ) => {
        #[tracing::instrument(level = "trace", skip(self $(, $skip)*), fields(volume = %self.volume_name))]
        fn $name(&self $(, $arg : $arg_ty )*) $(-> $ret)? {
            self.inner.load().$name($($arg),*)
        }
        forward_to_inner!($($rest)*);
    };
    (
        fn $name:ident(&self, ctx : $ctx_ty:ty $(, $arg:ident : $arg_ty:ty )*) $(-> $ret:ty)?;
        $($rest:tt)*
    ) => {
        #[tracing::instrument(level = "trace", skip(self, ctx), fields(volume = %self.volume_name))]
        fn $name(&self, ctx : $ctx_ty $(, $arg : $arg_ty )*) $(-> $ret)? {
            self.inner.load().$name(ctx $(, $arg)*)
        }
        forward_to_inner!($($rest)*);
    };
    (
        fn $name:ident(&self $(, $arg:ident : $arg_ty:ty )*) $(-> $ret:ty)?;
        $($rest:tt)*
    ) => {
        #[tracing::instrument(level = "trace", skip(self), fields(volume = %self.volume_name))]
        fn $name(&self $(, $arg : $arg_ty )*) $(-> $ret)? {
            self.inner.load().$name($($arg),*)
        }
        forward_to_inner!($($rest)*);
    };
}
