// Helper macro to forward calls to the inner PassthroughFs
macro_rules! forward_to_inner_full_trace {
    ($( fn $name:ident(&self $(, $arg:ident : $arg_ty:ty )*) $(-> $ret:ty)?; )*) => {
        $(
            fn $name(&self $(, $arg : $arg_ty )*) $(-> $ret)? {
                tracing::trace!("Calling {} with args: {:?}", stringify!($name), ($(&$arg),*));
                self.inner.$name($($arg),*)
            }
        )*
    };
}

macro_rules! forward_to_inner {
    ($( fn $name:ident(&self $(, $arg:ident : $arg_ty:ty )*) $(-> $ret:ty)?; )*) => {
        $(
            fn $name(&self $(, $arg : $arg_ty )*) $(-> $ret)? {
                tracing::trace!("Calling {}", stringify!($name));
                self.inner.$name($($arg),*)
            }
        )*
    };
}
