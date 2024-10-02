use proc_macro::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{
    braced, parse_macro_input, AngleBracketedGenericArguments, Error, GenericArgument, Ident, Path,
    Result, Token, Type, TypePath, Visibility,
};

struct AuthMacroInput {
    vis: Visibility,
    name: Ident,
    authority: Path,
    authenticators: Vec<(Ident, Path)>,
}

impl Parse for AuthMacroInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let vis = input.parse()?;
        let name: Path = input.parse()?;

        if name.leading_colon.is_some() || name.segments.len() != 1 {
            return Err(Error::new(name.span(), "Expected a name, not a path"));
        }

        let iden = &name.segments[0].ident;
        let (name, authority) =
            match &name.segments[0].arguments {
                syn::PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                    args, ..
                }) if args.len() == 1 => match &args[0] {
                    GenericArgument::Type(Type::Path(TypePath { path, .. })) => {
                        (iden.clone(), path.clone())
                    }
                    _ => {
                        return Err(Error::new(
                            name.span(),
                            "Expected the authority type to be a well-defined path.",
                        ))
                    }
                },
                _ => {
                    return Err(Error::new(
                        name.span(),
                        "A single parameter for the authority type is expected",
                    ))
                }
            };

        let content;
        braced!(content in input);
        let mut authenticators: Vec<_> = vec![];

        fn capitalize_first_letter(s: &str) -> String {
            let mut c = s.chars();
            match c.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
            }
        }

        for p in content
            .parse_terminated(Path::parse, Token![,])?
            .into_iter()
        {
            let id = p
                .clone()
                .segments
                .into_iter()
                .map(|s| {
                    s.ident
                        .to_string()
                        .split("_")
                        .map(capitalize_first_letter)
                        .collect::<Vec<_>>()
                        .concat()
                })
                .collect::<Vec<_>>()
                .concat();
            let id = Ident::new(&id, Span::call_site().into());

            authenticators.push((id.clone(), p));
        }

        if input.peek(Token![;]) {
            let _: Token![;] = input.parse()?;
        }

        Ok(AuthMacroInput {
            vis,
            name,
            authority,
            authenticators,
        })
    }
}

#[proc_macro]
pub fn composite_authenticator(input: TokenStream) -> TokenStream {
    let AuthMacroInput {
        vis,
        name,
        authority,
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
        .map(|(id, path)| {
            quote! {
                #id(<#path as Authenticator>::DeviceAttestation)
            }
        })
        .collect::<Vec<_>>();

    let device_variants = authenticators
        .clone()
        .into_iter()
        .map(|(id, path)| {
            quote! {
                #id(<#path as Authenticator>::Device)
            }
        })
        .collect::<Vec<_>>();

    let credential_variants = authenticators
        .clone()
        .into_iter()
        .map(|(id, path)| {
            quote! {
                #id(<<#path as Authenticator>::Device as UserAuthenticator>::Credential)
            }
        })
        .collect::<Vec<_>>();

    let match_attestations = authenticators.clone().into_iter().map(|(id, p)| {
        quote! {
            #device_attestation::#id(attestation) => {
                #device::#id(#p::verify_device(attestation)?)
            }
        }
    });

    let match_device_id_from_attestation = authenticators.clone().into_iter().map(|(id, _)| {
        quote! {
            #device_attestation::#id(attestation) => attestation.device_id()
        }
    });

    let match_device_id_from_device = authenticators.clone().into_iter().map(|(id, _)| {
        quote! {
            #device::#id(device) => device.device_id()
        }
    });

    let match_credentials = authenticators.clone().into_iter().map(|(id, _)| {
        quote! {
            (
                #device::#id(device),
                #credential::#id(credential),
            ) => device.verify_user(credential)
        }
    });
    let match_user_id = authenticators.clone().into_iter().map(|(id, _)| {
        quote! {
            #credential::#id(credential) => credential.user_id()
        }
    });

    // Generate the full struct and impl code
    let expanded = quote! {
        use fc_traits_authn::composite_prelude::*;

        #vis struct #auth_struct;

        impl Authenticator for #auth_struct {
            type Authority = #authority;
            type Challenger = Self;
            type DeviceAttestation = #device_attestation;
            type Device = #device;

            fn verify_device(attestation: Self::DeviceAttestation) -> Option<Self::Device> {
                Some(match attestation {
                    #(#match_attestations),*
                })
            }

            fn unpack_device(_: Self::DeviceAttestation) -> Self::Device {
                unimplemented!("This method should not be called, instead call inner `unpack_device` on each authenticator")
            }
        }

        impl Challenger for #auth_struct {
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
                    #(#match_device_id_from_attestation),*
                }
            }
        }


        #[derive(TypeInfo, Encode, Decode, MaxEncodedLen)]
        pub enum #device {
            #(#device_variants),*
        }

        impl UserAuthenticator for #device {
            type Authority = #authority;
            type Challenger = #auth_struct;
            type Credential = #credential;

            fn verify_user(&self, credential: &Self::Credential) -> Option<()> {
                match (self, credential) {
                    #(#match_credentials),*,
                    _ => None,
                }
            }

            fn device_id(&self) -> &DeviceId {
                match self {
                    #(#match_device_id_from_device),*
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
                    #(#match_user_id),*
                }
            }
        }
    };

    TokenStream::from(expanded)
}
