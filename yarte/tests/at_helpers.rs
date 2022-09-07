#![cfg(feature = "json")]
mod json {
    use serde::Serialize;
    use yarte::{Serialize as YSerialize, Template, TemplateText};
    #[derive(Serialize, YSerialize, Clone, Copy)]
    struct Json {
        f: usize,
    }

    #[derive(Serialize, YSerialize)]
    struct JsonN {
        f: usize,
    }

    #[derive(Template)]
    #[template(src = "{{ @json f }}")]
    struct JsonTemplate {
        f: Json,
    }

    #[derive(Template)]
    #[template(src = "{{ @json_pretty f }}")]
    struct JsonPrettyTemplate {
        f: Json,
    }

    #[derive(TemplateText)]
    #[template(src = "{{ @json f }}")]
    struct JsonTemplateT {
        f: Json,
    }

    #[derive(TemplateText)]
    #[template(src = "{{ @json_pretty f }}")]
    struct JsonPrettyTemplateT {
        f: Json,
    }

    #[test]
    fn json() {
        let f = Json { f: 1 };
        let t = JsonTemplate { f };
        assert_eq!(serde_json::to_string(&f).unwrap(), t.call().unwrap());

        let t = JsonPrettyTemplate { f };
        assert_eq!(serde_json::to_string_pretty(&f).unwrap(), t.call().unwrap());

        let t = JsonTemplateT { f };
        assert_eq!(serde_json::to_string(&f).unwrap(), t.call().unwrap());

        let t = JsonPrettyTemplateT { f };
        assert_eq!(serde_json::to_string_pretty(&f).unwrap(), t.call().unwrap());
    }

    #[cfg(feature = "fixed")]
    mod fixed {
        use super::*;
        use std::mem::MaybeUninit;
        use yarte::{TemplateFixed, TemplateFixedText};

        #[derive(TemplateFixed)]
        #[template(src = "{{ @json f }}")]
        struct JsonTemplateF {
            f: Json,
        }

        #[derive(TemplateFixed)]
        #[template(src = "{{ @json_pretty f }}")]
        struct JsonPrettyTemplateF {
            f: Json,
        }

        #[derive(TemplateFixedText)]
        #[template(src = "{{ @json &&&f }}", print = "code")]
        struct JsonTemplateFT {
            f: Json,
        }

        #[derive(TemplateFixedText)]
        #[template(src = "{{ @json_pretty f }}")]
        struct JsonPrettyTemplateFT {
            f: Json,
        }

        #[test]
        fn json() {
            let f = Json { f: 1 };
            let t = JsonTemplateF { f };

            assert_eq!(
                serde_json::to_string(&f).unwrap().as_bytes(),
                unsafe { t.call(&mut [MaybeUninit::uninit(); 1024]) }.unwrap()
            );

            let t = JsonPrettyTemplateF { f };
            assert_eq!(
                serde_json::to_string_pretty(&f).unwrap().as_bytes(),
                unsafe { t.call(&mut [MaybeUninit::uninit(); 1024]) }.unwrap()
            );

            let t = JsonTemplateFT { f };
            assert_eq!(
                serde_json::to_string(&f).unwrap().as_bytes(),
                unsafe { t.call(&mut [MaybeUninit::uninit(); 1024]) }.unwrap()
            );

            let t = JsonPrettyTemplateFT { f };
            assert_eq!(
                serde_json::to_string_pretty(&f).unwrap().as_bytes(),
                unsafe { t.call(&mut [MaybeUninit::uninit(); 1024]) }.unwrap()
            );
        }
    }

    #[cfg(feature = "bytes-buf")]
    mod bytes_buf {
        use super::*;
        use yarte::{TemplateBytes, TemplateBytesText};

        #[derive(TemplateBytes)]
        #[template(src = "{{ @json f }}")]
        struct JsonTemplateF {
            f: Json,
        }

        #[derive(TemplateBytes)]
        #[template(src = "{{ @json f }}")]
        struct JsonTemplateN {
            f: JsonN,
        }

        #[derive(TemplateBytesText)]
        #[template(src = "{{ @json f }}", print = "code")]
        struct JsonTemplateFT {
            f: Json,
        }

        #[test]
        fn json() {
            let f = Json { f: 1 };
            let t = JsonTemplateF { f };

            assert_eq!(serde_json::to_string(&f).unwrap(), t.ccall::<String>(0));

            let t = JsonTemplateFT { f };
            assert_eq!(serde_json::to_string(&f).unwrap(), t.ccall::<String>(0));

            let t = JsonTemplateN { f: JsonN { f: 1 } };
            assert_eq!(serde_json::to_string(&f).unwrap(), t.ccall::<String>(0));
        }
    }
}
