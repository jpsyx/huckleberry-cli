//! The one macro this crate defines.

/// Declares a Huckleberry string enum: a fixed set of spellings plus an
/// `Other` variant for anything else.
///
/// The Python client models these as `Literal[...]`, which is strict: a poo
/// colour Huckleberry adds next year makes the whole read fail validation.
/// That is the wrong trade for a client somebody depends on. Here an
/// unrecognized value deserializes to `Other`, so a single unfamiliar row
/// cannot cost a caller a month of history, and round-trips unchanged so a
/// value this crate does not understand is never quietly rewritten.
///
/// [`std::str::FromStr`] is the strict door, and it is the one a command-line
/// argument should come through: a person who types `--color purpel` wants to
/// be told, not to have it stored.
macro_rules! string_enum {
    (
        $(#[$enum_meta:meta])*
        $name:ident {
            $( $(#[$variant_meta:meta])* $variant:ident => $text:literal ),+ $(,)?
        }
    ) => {
        $(#[$enum_meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum $name {
            $( $(#[$variant_meta])* $variant, )+
            /// A value this crate does not have a name for. It is carried
            /// through reads and writes exactly as it arrived.
            ///
            /// Named `Unknown` rather than `Other` because `Other` is itself a
            /// real Huckleberry value: a bottle can hold "Other".
            Unknown(String),
        }

        impl $name {
            /// Every spelling this crate knows, in declaration order. This is
            /// the list to offer somebody who is choosing one.
            pub const NAMES: &'static [&'static str] = &[$($text),+];

            /// The spelling Huckleberry stores.
            #[must_use]
            pub fn as_str(&self) -> &str {
                match self {
                    $( Self::$variant => $text, )+
                    Self::Unknown(text) => text.as_str(),
                }
            }

            /// Reads a value off the wire. Anything unrecognized becomes
            /// [`Self::Other`] rather than a failure.
            #[must_use]
            pub fn from_wire(text: &str) -> Self {
                match text {
                    $( $text => Self::$variant, )+
                    other => Self::Unknown(other.to_owned()),
                }
            }

            /// Whether this is a spelling the crate knows.
            #[must_use]
            pub const fn is_known(&self) -> bool {
                !matches!(self, Self::Unknown(_))
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl ::core::str::FromStr for $name {
            type Err = crate::error::Error;

            /// The strict reading, for a value a person typed.
            fn from_str(text: &str) -> ::core::result::Result<Self, Self::Err> {
                let parsed = Self::from_wire(text);
                if parsed.is_known() {
                    ::core::result::Result::Ok(parsed)
                } else {
                    ::core::result::Result::Err(crate::error::Error::Invalid(format!(
                        "`{text}` is not one of: {}",
                        Self::NAMES.join(", ")
                    )))
                }
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(
                &self,
                serializer: S,
            ) -> ::core::result::Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(
                deserializer: D,
            ) -> ::core::result::Result<Self, D::Error> {
                let text = <::std::string::String as ::serde::Deserialize>::deserialize(deserializer)?;
                ::core::result::Result::Ok(Self::from_wire(&text))
            }
        }
    };
}
