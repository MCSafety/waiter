use proc_macro2::TokenStream;
use quote::ToTokens;

use syn::{GenericParam, ItemImpl, ItemStruct, Path, Type, ItemFn, ReturnType, Error};
use std::ops::Deref;
use crate::component::{generate_dependencies_create_code, generate_inject_dependencies_tuple};
use syn::spanned::Spanned;
use crate::attr_parser::ProvidesAttr;
use crate::component::type_to_inject::TypeToInject;

pub(crate) fn generate_component_provider_impl_struct(component: ItemStruct) -> TokenStream {
    let comp_name = component.ident;
    let comp_generics = component.generics.clone();

    let create_component_code = quote::quote! {
        #comp_name::__waiter_create(self)
    };
    let inject_deferred_code = quote::quote! {
        #comp_name::__waiter_inject_deferred(self, &component);
    };

    generate_component_provider_impl(
        quote::quote! { #comp_name #comp_generics },
        component.generics.params.iter().collect(),
        vec!(),
        create_component_code,
        inject_deferred_code
    )
}

pub(crate) fn generate_component_provider_impl_fn(
    provides: ProvidesAttr,
    factory: ItemFn,
    force_type: TokenStream
) -> Result<TokenStream, Error> {
    let comp_name = if force_type.is_empty() {
        let ret_value = if let ReturnType::Type(_, type_) = &factory.sig.output {
            if let Type::Path(type_path) = type_.deref() {
                type_path.path.segments.to_token_stream()
            } else {
                return Err(Error::new(
                    factory.span(),
                    "Unsupported return type for factory function"
                ))
            }
        } else {
            return Err(Error::new(
                factory.span(),
                "Return type must be specified for factory function"
            ))
        };
        ret_value
    } else {
        force_type.clone()
    };

    let fn_name = factory.sig.ident.to_token_stream();
    let fn_name_prefix = if force_type.is_empty() {
        force_type
    } else {
        quote::quote! { #force_type :: }
    };

    let dependencies_code = generate_dependencies_create_code(
        factory.sig.inputs.iter()
            .map(|arg| TypeToInject::from_fn_arg(arg.clone()))
            .collect::<Result<Vec<_>, _>>()?
    );
    let factory_code = generate_inject_dependencies_tuple(factory.sig.inputs.len());

    let create_component_code = quote::quote! {
        {
            let container = self;
            #dependencies_code
            #fn_name_prefix #fn_name #factory_code
        }
    };
    let inject_deferred_code = quote::quote! {};

    Ok(generate_component_provider_impl(
        comp_name,
        factory.sig.generics.params.iter()
            .filter(|p| if let GenericParam::Lifetime(_) = p { true } else { false })
            .collect(),
        provides.profiles,
        create_component_code,
        inject_deferred_code
    ))
}

pub fn generate_component_provider_impl(
    comp_name: TokenStream,
    comp_generics: Vec<&GenericParam>,
    profiles: Vec<Path>,
    create_component_code: TokenStream,
    inject_deferred_code: TokenStream
) -> TokenStream {
    let (profiles, provider_generics) = if profiles.is_empty() {
        let generic_profile = quote::quote! { PROFILE };

        let provider_generics = if comp_generics.is_empty() {
            quote::quote! { <PROFILE> }
        } else {
            quote::quote! { <#(#comp_generics),*, PROFILE> }
        };

        (vec!(generic_profile), provider_generics)
    } else {
        let profiles = profiles.iter()
            .map(|p| p.to_token_stream())
            .collect();
        (profiles, quote::quote! { <#(#comp_generics),*> })
    };

    let result = quote::quote! {#(
        impl #provider_generics ambient::Provider<#comp_name> for ambient::Container<#profiles> {
            type Impl = #comp_name;
            fn get(&self) -> ambient::Wrc<Self::Impl> {
                let type_id = std::any::TypeId::of::<#comp_name>();

                if !self.components.read().unwrap().contains_key(&type_id) {
					// This is outside write lock, because #create_component_code can call get again and would deadlock trying to get a read lock while we are still holding
					// the write lock. Outside the write lock multiple creations of the component might happen, but only one of the will be inserted. 
					let component = ambient::Wrc::new(#create_component_code);
					let mut comps = self.components.write().unwrap();
					if !comps.contains_key(&type_id) {
						comps.insert(type_id, component.clone());
						// #inject_deferred_code can call get again and would deadlock trying to get a read lock while we are still holding the write lock
						std::mem::drop(comps);
						#inject_deferred_code
					}
                }
				
				let comps = self.components.read().unwrap();
                let any = comps.get(&type_id).unwrap();

                any.clone()
                    .downcast::<#comp_name>()
                    .unwrap()
            }
            fn create(&self) -> Self::Impl {
                let component = #create_component_code;
                #inject_deferred_code
                component
            }
        }
    )*};

    return TokenStream::from(result);
}

pub(crate) fn generate_interface_provider_impl(provides: ProvidesAttr, impl_block: ItemImpl) -> TokenStream {
    let interface = match impl_block.trait_ {
        Some((_, interface, _)) => interface,
        None => return TokenStream::from(Error::new(
            impl_block.span(),
            "#[interface] can be used only on impl blocks for traits"
        ).to_compile_error())
    };

    let comp_name = if let Type::Path(comp_path) = *impl_block.self_ty {
        comp_path.path.segments.first().unwrap().ident.clone()
    } else {
        return TokenStream::from(Error::new(impl_block.self_ty.span(), "Failed to create provider").to_compile_error())
    };

    let provider_body = quote::quote! {{
        type Impl = #comp_name;
        fn get(&self) -> ambient::Wrc<Self::Impl> {
            ambient::Provider::<#comp_name>::get(self)
        }
        fn create(&self) -> Self::Impl {
            ambient::Provider::<#comp_name>::create(self)
        }
    }};

    let profiles = provides.profiles;
    let result = if profiles.is_empty() {
        quote::quote! {
            impl<P> ambient::Provider<dyn #interface> for ambient::Container<P> #provider_body
        }
    } else {
        quote::quote! {
            #(impl ambient::Provider<dyn #interface> for ambient::Container<#profiles> #provider_body)*
        }
    };

    return TokenStream::from(result);
}