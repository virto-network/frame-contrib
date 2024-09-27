use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{braced, parse_macro_input, Ident, Result, Token, Visibility};

struct AuthMacroInput {
    vis: Visibility,
    name: Ident,
    authenticators: Vec<Ident>,
}

impl Parse for AuthMacroInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let vis = input.parse()?;
        let name = input.parse()?;
        let _: Token![=] = input.parse()?;

        let content;
        braced!(content in input);
        let authenticators = content
            .parse_terminated(Ident::parse, Token![,])?
            .into_iter()
            .collect();

        let _: Token![;] = input.parse()?;

        Ok(AuthMacroInput {
            vis,
            name,
            authenticators,
        })
    }
}

#[proc_macro]
pub fn composite_authenticators(input: TokenStream) -> TokenStream {
    let AuthMacroInput {
        vis,
        name,
        authenticators,
    } = parse_macro_input!(input as AuthMacroInput);

    // Dynamically create identifiers based on `name`
    let auth_struct = format_ident!("{}Authenticator", name);
    let device_attestation = format_ident!("{}DeviceAttestation", name);
    let device = format_ident!("{}Device", name);
    let credential = format_ident!("{}Credential", name);

    // Generate enum variants for authenticators
    let auth_variants = authenticators
        .clone()
        .into_iter()
        .map(|auth| {
            let variant_name = format_ident!("{}", auth);
            quote! {
                #variant_name(<#variant_name as Authenticator>::DeviceAttestation)
            }
        })
        .collect::<Vec<_>>();

    let device_variants = authenticators
        .clone()
        .into_iter()
        .map(|auth| {
            let variant_name = format_ident!("{}", auth);
            quote! {
                #variant_name(<#variant_name as Authenticator>::Device)
            }
        })
        .collect::<Vec<_>>();

    let credential_variants = authenticators.clone().into_iter().map(|auth| {
        let variant_name = format_ident!("{}", auth);
        quote! {
            #variant_name(<<#variant_name as Authenticator>::Device as UserAuthenticator>::Credential)
        }
    }).collect::<Vec<_>>();

    // Generate the full struct and impl code
    let expanded = quote! {
        use fc_traits_authn::*;
        use frame_support::pallet_prelude::Get;

        #vis struct #auth_struct<A>(core::marker::PhantomData<A>);

        impl<Authority: Get<AuthorityId> + 'static> Authenticator for #auth_struct<Authority> {
            type Authority = Authority;
            type Challenger = Self;
            type DeviceAttestation = #device_attestation;
            type Device = #device<Authority>;

            fn verify_device(attestation: Self::DeviceAttestation) -> Option<Self::Device> {
                Some(match attestation {
                    #(#device_attestation::#authenticators(attestation) => {
                        #device::#authenticators(#authenticators::verify_device(attestation)?)
                    }),*
                })
            }

            fn unpack_device(_: Self::DeviceAttestation) -> Self::Device {
                unimplemented!("This method should not be called, instead call inner `unpack_device` on each authenticator")
            }
        }

        impl<Authority> Challenger for #auth_struct<Authority> {
            type Context = ();

            fn generate(_: &Self::Context) -> Challenge {
                unimplemented!(
                    "This method should not be called, instead call inner `generate` on each challenger"
                )
            }
        }

        #[derive(TypeInfo, DebugNoBound, EqNoBound, PartialEq, Clone, Encode, Decode)]
        pub enum #device_attestation {
            #(#auth_variants),*
        }

        impl DeviceChallengeResponse<()> for #device_attestation {
            fn is_valid(&self) -> bool {
                unimplemented!(
                    "This method should not be called, instead call inner `is_valid` on each attestation"
                )
            }

            fn used_challenge(&self) -> ((), Challenge) {
                unimplemented!(
                    "This method should not be called, instead call inner `used_challenge` on each attestation"
                )
            }

            fn authority(&self) -> AuthorityId {
                unimplemented!(
                    "This method should not be called, instead call inner `authority` on each attestation"
                )
            }

            fn device_id(&self) -> &DeviceId {
                match self {
                    #(#device_attestation::#authenticators(attestation) => attestation.device_id()),*
                }
            }
        }


        #[derive(TypeInfo, Encode, Decode, MaxEncodedLen)]
        #[scale_info(skip_type_params(Authority))]
        pub enum #device<Authority> {
            #[allow(non_camel_case_types)]
            __phantom(std::marker::PhantomData<Authority>),
            #(#device_variants),*
        }

        impl<Authority: Get<AuthorityId> + 'static> UserAuthenticator for #device<Authority> {
            type Authority = Authority;
            type Challenger = #auth_struct<Authority>;
            type Credential = #credential;

            fn verify_user(&self, credential: &Self::Credential) -> Option<()> {
                match (self, credential) {
                    #((
                        #device::#authenticators(device),
                        #credential::#authenticators(credential),
                    ) => device.verify_user(credential)),*,
                    _ => None,

                }
            }

            fn device_id(&self) -> &DeviceId {
                match self {
                    #(#device::#authenticators(device) => device.device_id()),*,
                    #device::__phantom(_) => {
                        unimplemented!("__phantom should not be a valid device descriptor")
                    },

                }
            }
        }


        #[derive(TypeInfo, DebugNoBound, EqNoBound, PartialEq, Clone, Encode, Decode, MaxEncodedLen)]
        pub enum #credential {
            #(#credential_variants),*
        }

        impl UserChallengeResponse<()> for #credential {
            fn is_valid(&self) -> bool {
                unimplemented!(
                    "This method should not be called, instead call inner `is_valid` on each credential"
                )
            }

            fn used_challenge(&self) -> ((), Challenge) {
                unimplemented!(
                    "This method should not be called, instead call inner `used_challenge` on each credential"
                )
            }

            fn authority(&self) -> AuthorityId {
                unimplemented!(
                    "This method should not be called, instead call inner `authority` on each credential"
                )
            }

            fn user_id(&self) -> HashedUserId {
                match self {
                    #(#credential::#authenticators(credential) => credential.user_id()),*
                }
            }
        }
    };

    TokenStream::from(expanded)
}
