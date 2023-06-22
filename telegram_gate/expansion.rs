mod state {
    //! Contains stronlgy-typed states of the [`Dialogue`](super::Dialogue).
    #![allow(clippy::non_ascii_literal)]
    use super::{command, Command};
    use async_trait::async_trait;
    use derive_more::{From, TryInto};
    use std::convert::Infallible;
    use teloxide::{types::ChatId, Bot};
    /// Error struct for [`MakeTransition::make_transition()`] function,
    /// containing error target and reason of failure.
    #[error("Transition failed")]
    pub struct FailedTransition<T> {
        /// Error target of transition.
        pub target: T,
        /// Failure reason.
        #[source]
        pub reason: eyre::Report,
    }
    #[automatically_derived]
    impl<T: ::core::fmt::Debug> ::core::fmt::Debug for FailedTransition<T> {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_struct_field2_finish(
                f,
                "FailedTransition",
                "target",
                &self.target,
                "reason",
                &&self.reason,
            )
        }
    }
    #[allow(unused_qualifications)]
    impl<T> std::error::Error for FailedTransition<T>
    where
        Self: std::fmt::Debug + std::fmt::Display,
    {
        fn source(&self) -> std::option::Option<&(dyn std::error::Error + 'static)> {
            use thiserror::__private::AsDynError;
            std::option::Option::Some(self.reason.as_dyn_error())
        }
    }
    #[allow(unused_qualifications)]
    impl<T> std::fmt::Display for FailedTransition<T> {
        #[allow(clippy::used_underscore_binding)]
        fn fmt(&self, __formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            #[allow(unused_variables, deprecated)]
            let Self { target, reason } = self;
            __formatter.write_fmt(format_args!("Transition failed"))
        }
    }
    impl<T> FailedTransition<T> {
        pub fn from_err<E: Into<eyre::Report>>(target: T, error: E) -> Self {
            Self {
                target,
                reason: error.into(),
            }
        }
        pub fn into_boxed(self) -> FailedTransition<StateBox>
        where
            T: Into<StateBox>,
        {
            FailedTransition {
                target: self.target.into(),
                reason: self.reason,
            }
        }
    }
    /// Trait to make a transition from one state to another.
    ///
    /// # Generics
    ///
    /// - `T` - means end *target* state of successfull transition.
    /// - `B` - means an event *by* which transition is possible.
    ///
    /// Transition will return `Self::ErrorTarget` as an error target if transition failed.
    pub trait MakeTransition<T, B> {
        /// Target which will be returned on failed transition attempt.
        type ErrorTarget;
        /// Try to perfrom a transition from [`Self`] to [`Self::Target`].
        ///
        /// Rerturns possibly different state with fail reason if not succeed.
        ///
        /// # Errors
        ///
        /// Fails if failed to perform a transition. Concrete error depends on the implementation.
        #[must_use]
        #[allow(clippy::type_complexity, clippy::type_repetition_in_bounds)]
        fn make_transition<'async_trait>(
            self,
            by: B,
            bot: Bot,
            chat_id: ChatId,
        ) -> ::core::pin::Pin<
            Box<
                dyn ::core::future::Future<Output = Result<T, FailedTransition<Self::ErrorTarget>>>
                    + ::core::marker::Send
                    + 'async_trait,
            >,
        >
        where
            Self: 'async_trait;
    }
    #[allow(clippy::module_name_repetitions)]
    pub enum StateBox {
        Unauthorized(Unauthorized<unauthorized::KindBox>),
        Authorized(Authorized),
    }
    #[automatically_derived]
    #[allow(clippy::module_name_repetitions)]
    impl ::core::fmt::Debug for StateBox {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match self {
                StateBox::Unauthorized(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(f, "Unauthorized", &__self_0)
                }
                StateBox::Authorized(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(f, "Authorized", &__self_0)
                }
            }
        }
    }
    #[automatically_derived]
    #[allow(clippy::module_name_repetitions)]
    impl ::core::clone::Clone for StateBox {
        #[inline]
        fn clone(&self) -> StateBox {
            match self {
                StateBox::Unauthorized(__self_0) => {
                    StateBox::Unauthorized(::core::clone::Clone::clone(__self_0))
                }
                StateBox::Authorized(__self_0) => {
                    StateBox::Authorized(::core::clone::Clone::clone(__self_0))
                }
            }
        }
    }
    #[automatically_derived]
    impl ::core::convert::From<(Unauthorized<unauthorized::KindBox>)> for StateBox {
        #[inline]
        fn from(original: (Unauthorized<unauthorized::KindBox>)) -> StateBox {
            StateBox::Unauthorized(original)
        }
    }
    #[automatically_derived]
    impl ::core::convert::From<(Authorized)> for StateBox {
        #[inline]
        fn from(original: (Authorized)) -> StateBox {
            StateBox::Authorized(original)
        }
    }
    impl ::core::convert::TryFrom<StateBox> for (Unauthorized<unauthorized::KindBox>) {
        type Error = &'static str;
        #[allow(unused_variables)]
        #[inline]
        fn try_from(value: StateBox) -> ::core::result::Result<Self, Self::Error> {
            match value { StateBox :: Unauthorized (__0) => :: core :: result :: Result :: Ok (__0) , _ => :: core :: result :: Result :: Err ("Only Unauthorized can be converted to Unauthorized < unauthorized :: KindBox >") , }
        }
    }
    impl ::core::convert::TryFrom<StateBox> for (Authorized) {
        type Error = &'static str;
        #[allow(unused_variables)]
        #[inline]
        fn try_from(value: StateBox) -> ::core::result::Result<Self, Self::Error> {
            match value {
                StateBox::Authorized(__0) => ::core::result::Result::Ok(__0),
                _ => ::core::result::Result::Err("Only Authorized can be converted to Authorized"),
            }
        }
    }
    impl Default for StateBox {
        fn default() -> Self {
            Self::Unauthorized(Unauthorized::default())
        }
    }
    impl MakeTransition<StateBox, Command> for StateBox {
        type ErrorTarget = StateBox;
        #[allow(
            clippy::async_yields_async,
            clippy::let_unit_value,
            clippy::no_effect_underscore_binding,
            clippy::shadow_same,
            clippy::type_complexity,
            clippy::type_repetition_in_bounds,
            clippy::used_underscore_binding
        )]
        fn make_transition<'async_trait>(
            self,
            cmd: Command,
            bot: Bot,
            chat_id: ChatId,
        ) -> ::core::pin::Pin<
            Box<
                dyn ::core::future::Future<Output = Result<Self, FailedTransition<Self>>>
                    + ::core::marker::Send
                    + 'async_trait,
            >,
        >
        where
            Self: 'async_trait,
        {
            Box::pin(async move {
                if let ::core::option::Option::Some(__ret) =
                    ::core::option::Option::None::<Result<Self, FailedTransition<Self>>>
                {
                    return __ret;
                }
                let __self = self;
                let cmd = cmd;
                let bot = bot;
                let chat_id = chat_id;
                let __ret: Result<Self, FailedTransition<Self>> = {
                    match __self {
                        Self::Unauthorized(unauthorized) => unauthorized
                            .make_transition(cmd, bot, chat_id)
                            .await
                            .map(Into::into)
                            .map_err(FailedTransition::into_boxed),
                        Self::Authorized(_) => ::core::panicking::panic("not yet implemented"),
                    }
                };
                #[allow(unreachable_code)]
                __ret
            })
        }
    }
    impl<'mes> MakeTransition<StateBox, &'mes str> for StateBox {
        type ErrorTarget = StateBox;
        #[allow(
            clippy::async_yields_async,
            clippy::let_unit_value,
            clippy::no_effect_underscore_binding,
            clippy::shadow_same,
            clippy::type_complexity,
            clippy::type_repetition_in_bounds,
            clippy::used_underscore_binding
        )]
        fn make_transition<'async_trait>(
            self,
            text: &'mes str,
            bot: Bot,
            chat_id: ChatId,
        ) -> ::core::pin::Pin<
            Box<
                dyn ::core::future::Future<Output = Result<Self, FailedTransition<Self>>>
                    + ::core::marker::Send
                    + 'async_trait,
            >,
        >
        where
            'mes: 'async_trait,
            Self: 'async_trait,
        {
            Box::pin(async move {
                if let ::core::option::Option::Some(__ret) =
                    ::core::option::Option::None::<Result<Self, FailedTransition<Self>>>
                {
                    return __ret;
                }
                let __self = self;
                let bot = bot;
                let chat_id = chat_id;
                let __ret: Result<Self, FailedTransition<Self>> = {
                    match __self {
                        Self::Unauthorized(unauthorized) => unauthorized
                            .make_transition(text, bot, chat_id)
                            .await
                            .map(Into::into)
                            .map_err(FailedTransition::into_boxed),
                        Self::Authorized(_) => ::core::panicking::panic("not yet implemented"),
                    }
                };
                #[allow(unreachable_code)]
                __ret
            })
        }
    }
    /// Unauthorized state. Corresponds to the beginning of the dialogue.
    ///
    /// User becomes [authorized](Authorized) when they submit the corresponding admin token.
    #[must_use]
    pub struct Unauthorized<K> {
        /// Secret token generated on every run.
        /// User should copy this token from logs and send to the bot in order to prove that they are the admin.
        pub admin_token: String,
        pub kind: K,
    }
    #[automatically_derived]
    impl<K: ::core::fmt::Debug> ::core::fmt::Debug for Unauthorized<K> {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_struct_field2_finish(
                f,
                "Unauthorized",
                "admin_token",
                &self.admin_token,
                "kind",
                &&self.kind,
            )
        }
    }
    #[automatically_derived]
    impl<K: ::core::clone::Clone> ::core::clone::Clone for Unauthorized<K> {
        #[inline]
        fn clone(&self) -> Unauthorized<K> {
            Unauthorized {
                admin_token: ::core::clone::Clone::clone(&self.admin_token),
                kind: ::core::clone::Clone::clone(&self.kind),
            }
        }
    }
    impl Default for Unauthorized<unauthorized::KindBox> {
        fn default() -> Self {
            Self {
                admin_token: String::from("qwerty"),
                kind: unauthorized::KindBox::default(),
            }
        }
    }
    impl MakeTransition<Unauthorized<unauthorized::KindBox>, Command>
        for Unauthorized<unauthorized::KindBox>
    {
        type ErrorTarget = Unauthorized<unauthorized::KindBox>;
        #[allow(
            clippy::async_yields_async,
            clippy::let_unit_value,
            clippy::no_effect_underscore_binding,
            clippy::shadow_same,
            clippy::type_complexity,
            clippy::type_repetition_in_bounds,
            clippy::used_underscore_binding
        )]
        fn make_transition<'async_trait>(
            self,
            cmd: Command,
            bot: Bot,
            chat_id: ChatId,
        ) -> ::core::pin::Pin<
            Box<
                dyn ::core::future::Future<Output = Result<Self, FailedTransition<Self>>>
                    + ::core::marker::Send
                    + 'async_trait,
            >,
        >
        where
            Self: 'async_trait,
        {
            Box::pin(async move {
                if let ::core::option::Option::Some(__ret) =
                    ::core::option::Option::None::<Result<Self, FailedTransition<Self>>>
                {
                    return __ret;
                }
                let __self = self;
                let cmd = cmd;
                let bot = bot;
                let chat_id = chat_id;
                let __ret: Result<Self, FailedTransition<Self>> = {
                    match (__self.kind, cmd) {
                        (unauthorized::KindBox::Start(start), Command::Start(start_cmd)) => {
                            <_ as MakeTransition<
                                Unauthorized<unauthorized::kind::Start>,
                                command::Start,
                            >>::make_transition(
                                Unauthorized {
                                    admin_token: __self.admin_token,
                                    kind: start,
                                },
                                start_cmd,
                                bot,
                                chat_id,
                            )
                            .await
                            .map(Into::into)
                            .map_err(
                                |_infallible: FailedTransition<Infallible>| {
                                    ::core::panicking::panic_fmt(format_args!("Infallible"));
                                },
                            )
                        }
                        (unauthorized::KindBox::WaitingForSecretPhrase(_), _) => {
                            ::core::panicking::panic("not yet implemented")
                        }
                        (_, Command::Help) => {
                            ::core::panicking::panic("internal error: entered unreachable code")
                        }
                    }
                };
                #[allow(unreachable_code)]
                __ret
            })
        }
    }
    impl<'mes> MakeTransition<Unauthorized<unauthorized::KindBox>, &'mes str>
        for Unauthorized<unauthorized::KindBox>
    {
        type ErrorTarget = Unauthorized<unauthorized::KindBox>;
        #[allow(
            clippy::async_yields_async,
            clippy::let_unit_value,
            clippy::no_effect_underscore_binding,
            clippy::shadow_same,
            clippy::type_complexity,
            clippy::type_repetition_in_bounds,
            clippy::used_underscore_binding
        )]
        fn make_transition<'async_trait>(
            self,
            text: &'mes str,
            bot: Bot,
            chat_id: ChatId,
        ) -> ::core::pin::Pin<
            Box<
                dyn ::core::future::Future<Output = Result<Self, FailedTransition<Self>>>
                    + ::core::marker::Send
                    + 'async_trait,
            >,
        >
        where
            'mes: 'async_trait,
            Self: 'async_trait,
        {
            Box::pin(async move {
                if let ::core::option::Option::Some(__ret) =
                    ::core::option::Option::None::<Result<Self, FailedTransition<Self>>>
                {
                    return __ret;
                }
                let __self = self;
                let bot = bot;
                let chat_id = chat_id;
                let __ret: Result<Self, FailedTransition<Self>> = {
                    match __self.kind {
                        unauthorized::KindBox::Start(start) => <_ as MakeTransition<
                            Unauthorized<unauthorized::kind::WaitingForSecretPhrase>,
                            &'mes str,
                        >>::make_transition(
                            Unauthorized {
                                admin_token: __self.admin_token,
                                kind: start,
                            },
                            text,
                            bot,
                            chat_id,
                        )
                        .await
                        .map(Into::into)
                        .map_err(|failed_transition| FailedTransition::<Self> {
                            target: failed_transition.target.into(),
                            reason: failed_transition.reason,
                        }),
                        unauthorized::KindBox::WaitingForSecretPhrase(_) => {
                            ::core::panicking::panic("not yet implemented")
                        }
                    }
                };
                #[allow(unreachable_code)]
                __ret
            })
        }
    }
    /// Auhtorized state.
    #[must_use]
    pub struct Authorized;
    #[automatically_derived]
    impl ::core::fmt::Debug for Authorized {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(f, "Authorized")
        }
    }
    #[automatically_derived]
    impl ::core::default::Default for Authorized {
        #[inline]
        fn default() -> Authorized {
            Authorized {}
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for Authorized {
        #[inline]
        fn clone(&self) -> Authorized {
            Authorized
        }
    }
    pub mod unauthorized {
        //! Module with [`Unauthorized`] states.
        use super::{
            async_trait, command, Bot, ChatId, FailedTransition, From, Infallible, MakeTransition,
            TryInto, Unauthorized,
        };
        use eyre::eyre;
        use teloxide::requests::Requester as _;
        /// Boxed sub-state of [`Unauthorized`].
        pub enum KindBox {
            Start(kind::Start),
            WaitingForSecretPhrase(kind::WaitingForSecretPhrase),
        }
        #[automatically_derived]
        impl ::core::fmt::Debug for KindBox {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                match self {
                    KindBox::Start(__self_0) => {
                        ::core::fmt::Formatter::debug_tuple_field1_finish(f, "Start", &__self_0)
                    }
                    KindBox::WaitingForSecretPhrase(__self_0) => {
                        ::core::fmt::Formatter::debug_tuple_field1_finish(
                            f,
                            "WaitingForSecretPhrase",
                            &__self_0,
                        )
                    }
                }
            }
        }
        #[automatically_derived]
        impl ::core::clone::Clone for KindBox {
            #[inline]
            fn clone(&self) -> KindBox {
                let _: ::core::clone::AssertParamIsClone<kind::Start>;
                let _: ::core::clone::AssertParamIsClone<kind::WaitingForSecretPhrase>;
                *self
            }
        }
        #[automatically_derived]
        impl ::core::marker::Copy for KindBox {}
        #[automatically_derived]
        impl ::core::convert::From<(kind::WaitingForSecretPhrase)> for KindBox {
            #[inline]
            fn from(original: (kind::WaitingForSecretPhrase)) -> KindBox {
                KindBox::WaitingForSecretPhrase(original)
            }
        }
        #[automatically_derived]
        impl ::core::convert::From<(kind::Start)> for KindBox {
            #[inline]
            fn from(original: (kind::Start)) -> KindBox {
                KindBox::Start(original)
            }
        }
        impl ::core::convert::TryFrom<KindBox> for (kind::Start) {
            type Error = &'static str;
            #[allow(unused_variables)]
            #[inline]
            fn try_from(value: KindBox) -> ::core::result::Result<Self, Self::Error> {
                match value {
                    KindBox::Start(__0) => ::core::result::Result::Ok(__0),
                    _ => {
                        ::core::result::Result::Err("Only Start can be converted to kind :: Start")
                    }
                }
            }
        }
        impl ::core::convert::TryFrom<KindBox> for (kind::WaitingForSecretPhrase) {
            type Error = &'static str;
            #[allow(unused_variables)]
            #[inline]
            fn try_from(value: KindBox) -> ::core::result::Result<Self, Self::Error> {
                match value { KindBox :: WaitingForSecretPhrase (__0) => :: core :: result :: Result :: Ok (__0) , _ => :: core :: result :: Result :: Err ("Only WaitingForSecretPhrase can be converted to kind :: WaitingForSecretPhrase") , }
            }
        }
        impl Default for KindBox {
            fn default() -> Self {
                Self::Start(kind::Start)
            }
        }
        pub mod kind {
            //! Module with [`Unauthorized`](Unauthorized) kinds.
            use super::{super::StateBox, KindBox, Unauthorized};
            /// Start of the dialog. Waiting for user signing in.
            pub struct Start;
            #[automatically_derived]
            impl ::core::fmt::Debug for Start {
                fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                    ::core::fmt::Formatter::write_str(f, "Start")
                }
            }
            #[automatically_derived]
            impl ::core::clone::Clone for Start {
                #[inline]
                fn clone(&self) -> Start {
                    *self
                }
            }
            #[automatically_derived]
            impl ::core::marker::Copy for Start {}
            /// Waiting for user to enter a secret phrase spawned in logs to prove that
            pub struct WaitingForSecretPhrase;
            #[automatically_derived]
            impl ::core::fmt::Debug for WaitingForSecretPhrase {
                fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                    ::core::fmt::Formatter::write_str(f, "WaitingForSecretPhrase")
                }
            }
            #[automatically_derived]
            impl ::core::clone::Clone for WaitingForSecretPhrase {
                #[inline]
                fn clone(&self) -> WaitingForSecretPhrase {
                    *self
                }
            }
            #[automatically_derived]
            impl ::core::marker::Copy for WaitingForSecretPhrase {}
            impl From<Unauthorized<Start>> for Unauthorized<KindBox> {
                fn from(value: Unauthorized<Start>) -> Self {
                    Self {
                        admin_token: value.admin_token,
                        kind: KindBox::from(value.kind),
                    }
                }
            }
            impl From<Unauthorized<Start>> for StateBox {
                fn from(value: Unauthorized<Start>) -> Self {
                    Unauthorized::<KindBox>::from(value).into()
                }
            }
            impl From<Unauthorized<WaitingForSecretPhrase>> for Unauthorized<KindBox> {
                fn from(value: Unauthorized<WaitingForSecretPhrase>) -> Self {
                    Self {
                        admin_token: value.admin_token,
                        kind: KindBox::from(value.kind),
                    }
                }
            }
            impl From<Unauthorized<WaitingForSecretPhrase>> for StateBox {
                fn from(value: Unauthorized<WaitingForSecretPhrase>) -> Self {
                    Unauthorized::<KindBox>::from(value).into()
                }
            }
        }
        impl MakeTransition<Self, command::Start> for Unauthorized<kind::Start> {
            type ErrorTarget = Infallible;
            #[allow(
                clippy::async_yields_async,
                clippy::let_unit_value,
                clippy::no_effect_underscore_binding,
                clippy::shadow_same,
                clippy::type_complexity,
                clippy::type_repetition_in_bounds,
                clippy::used_underscore_binding
            )]
            fn make_transition<'async_trait>(
                self,
                _start_cmd: command::Start,
                _bot: Bot,
                _chat_id: ChatId,
            ) -> ::core::pin::Pin<
                Box<
                    dyn ::core::future::Future<
                            Output = Result<Self, FailedTransition<Self::ErrorTarget>>,
                        > + ::core::marker::Send
                        + 'async_trait,
                >,
            >
            where
                Self: 'async_trait,
            {
                Box::pin(async move {
                    if let ::core::option::Option::Some(__ret) = ::core::option::Option::None::<
                        Result<Self, FailedTransition<Self::ErrorTarget>>,
                    > {
                        return __ret;
                    }
                    let __self = self;
                    let _start_cmd = _start_cmd;
                    let _bot = _bot;
                    let _chat_id = _chat_id;
                    let __ret: Result<Self, FailedTransition<Self::ErrorTarget>> = { Ok(__self) };
                    #[allow(unreachable_code)]
                    __ret
                })
            }
        }
        impl<'mes> MakeTransition<Unauthorized<kind::WaitingForSecretPhrase>, &'mes str>
            for Unauthorized<kind::Start>
        {
            type ErrorTarget = Self;
            #[allow(
                clippy::async_yields_async,
                clippy::let_unit_value,
                clippy::no_effect_underscore_binding,
                clippy::shadow_same,
                clippy::type_complexity,
                clippy::type_repetition_in_bounds,
                clippy::used_underscore_binding
            )]
            fn make_transition<'async_trait>(
                self,
                text: &'mes str,
                bot: Bot,
                chat_id: ChatId,
            ) -> ::core::pin::Pin<
                Box<
                    dyn ::core::future::Future<
                            Output = Result<
                                Unauthorized<kind::WaitingForSecretPhrase>,
                                FailedTransition<Self::ErrorTarget>,
                            >,
                        > + ::core::marker::Send
                        + 'async_trait,
                >,
            >
            where
                'mes: 'async_trait,
                Self: 'async_trait,
            {
                Box::pin(async move {
                    if let ::core::option::Option::Some(__ret) = ::core::option::Option::None::<
                        Result<
                            Unauthorized<kind::WaitingForSecretPhrase>,
                            FailedTransition<Self::ErrorTarget>,
                        >,
                    > {
                        return __ret;
                    }
                    let __self = self;
                    let bot = bot;
                    let chat_id = chat_id;
                    let __ret: Result<
                        Unauthorized<kind::WaitingForSecretPhrase>,
                        FailedTransition<Self::ErrorTarget>,
                    > = {
                        const SIGN_IN: &str = "🔐 Sign in";
                        if text != SIGN_IN {
                            return Err(FailedTransition {
                                target: __self,
                                reason: {
                                    let error = ::eyre::private::format_err(format_args!(
                                        "Expected `{0}` input, but `{1}` found",
                                        SIGN_IN, text
                                    ));
                                    error
                                },
                            });
                        }
                        match bot
                            .send_message(
                                chat_id,
                                "Please, enter the admin token spawned in server logs",
                            )
                            .await
                        {
                            Ok(ok) => ok,
                            Err(err) => return Err(FailedTransition::from_err(__self, err)),
                        };
                        Ok(Unauthorized {
                            admin_token: __self.admin_token,
                            kind: kind::WaitingForSecretPhrase,
                        })
                    };
                    #[allow(unreachable_code)]
                    __ret
                })
            }
        }
    }
}
