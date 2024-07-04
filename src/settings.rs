// use eframe::egui::{Key, KeyboardShortcut, ModifierNames, Modifiers};

// use knuffel::{
//     ast::{Literal, TypeName},
//     decode::{Context, Kind},
//     errors::{DecodeError, ExpectedType},
//     span::Spanned,
//     traits::ErrorSpan,
//     DecodeScalar,
// };

// // #[derive(knuffel::Decode, Debug, PartialEq, Eq)]
// // pub enum Command {
// //     Test,
// // }

// #[derive(knuffel::Decode, Debug, PartialEq, Eq)]
// pub struct Keybindings {
//     // #[knuffel(argument)]
//     // path: String,
//     #[knuffel(children(name = "bind"))]
//     bindings: Vec<Keybinding>,
// }

// #[derive(knuffel::Decode, Debug, PartialEq, Eq)]
// pub struct Keybinding {
//     #[knuffel(argument)]
//     shortcut: Shortcut,
//     #[knuffel(child)]
//     cmd: Command,
// }

// // #[derive(knuffel::Decode)]
// // struct Plugin {
// //     #[knuffel(argument)]
// //     name: String,
// //     #[knuffel(property)]
// //     url: String,
// // }

// #[derive(Debug, PartialEq, Eq)]
// pub struct Shortcut(pub KeyboardShortcut);

// impl<S: ErrorSpan> DecodeScalar<S> for Shortcut {
//     fn raw_decode(
//         val: &Spanned<Literal, S>,
//         ctx: &mut Context<S>,
//     ) -> Result<Shortcut, DecodeError<S>> {
//         match &**val {
//             Literal::String(ref s) => {
//                 let parts: Vec<_> = s.split(' ').collect();

//                 let modifiers = parts
//                     .iter()
//                     .flat_map(|s| try_parse_modifier(*s))
//                     .fold(Modifiers::NONE, |modifiers, modifier| modifiers | modifier);

//                 let non_modifiers: Vec<_> = parts
//                     .iter()
//                     .filter(|k| try_parse_modifier(k).is_none())
//                     .collect();

//                 if non_modifiers.len() != 1 {
//                     return Err(DecodeError::conversion(
//                         val,
//                         format!("There has to be exectly one keyboard key"),
//                     ));
//                 }

//                 let key = non_modifiers[0];
//                 let Some(key) = Key::from_name(key) else {
//                     return Err(DecodeError::conversion(
//                         val,
//                         format!("{key} is not a valid keyboard key or modifier look at TODO url for the complete list"),
//                     ));
//                 };

//                 Ok(Shortcut(KeyboardShortcut::new(modifiers, key)))
//             }
//             _ => {
//                 ctx.emit_error(DecodeError::scalar_kind(Kind::String, val));
//                 Ok(Shortcut(KeyboardShortcut::new(Modifiers::NONE, Key::Enter)))
//             }
//         }
//     }
//     fn type_check(type_name: &Option<Spanned<TypeName, S>>, ctx: &mut Context<S>) {
//         if let Some(typ) = type_name {
//             ctx.emit_error(DecodeError::TypeName {
//                 span: typ.span().clone(),
//                 //found: Some(typ.clone()),
//                 found: None,
//                 expected: ExpectedType::no_type(),
//                 rust_type: "KeyboardShortcut",
//             });
//         }
//     }
// }
// fn try_parse_modifier(mod_str: &str) -> Option<Modifiers> {
//     match mod_str {
//         s if s == ModifierNames::NAMES.alt => Some(Modifiers::ALT),
//         s if s == ModifierNames::NAMES.ctrl => Some(Modifiers::CTRL),
//         s if s == ModifierNames::NAMES.mac_cmd => Some(Modifiers::MAC_CMD),
//         s if s == ModifierNames::NAMES.mac_alt => Some(Modifiers::ALT),
//         _ => None,
//     }
// }

#[cfg(test)]
mod tests {

    use super::*;
    #[test]
    pub fn test_settings_parsing() {
        use kdl::KdlDocument;

        let doc_str = r#"
        hello 1 2 3

        world prop="value" {
            child 1
            child 2
        }
        "#;

        let doc: KdlDocument = doc_str.parse().expect("failed to parse KDL");

        assert_eq!(doc.get_args("hello"), vec![&1.into(), &2.into(), &3.into()]);

        assert_eq!(
            doc.get("world").map(|node| &node["prop"]),
            Some(&"value".into())
        );

        // Documents fully roundtrip:
        assert_eq!(doc.to_string(), doc_str);
    }
}
