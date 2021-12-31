/* #[macro_export]
macro_rules! inject {
    ($comp:path: $($profile:path),*) => {
        {
            $(
                if profile_name::<$profile>().eq(&ambient::APP_PROFILE.as_str()) {
                    ambient::Provider::<$comp>::create(&mut ambient::Container::<$profile>::new())
                } else
            )*
            { ambient::Provider::<$comp>::create(&mut ambient::Container::<ambient::profiles::Default>::new()) }
        }
    }
}

#[macro_export]
macro_rules! wrap {
    ($wrapped_type:path as $wrapper_name:ident) => {
        pub struct $wrapper_name($wrapped_type);
        impl Deref for $wrapper_name {
            type Target = $wrapped_type;
            fn deref(&self) -> &Self::Target {
                return &self.0;
            }
        }
    }
}
*/