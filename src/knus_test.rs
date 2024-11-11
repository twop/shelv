use miette::IntoDiagnostic;

#[derive(knus::Decode, Debug)]
enum TopLevelNode {
    #[knus(name = "Route")]
    Route(Route),
    #[knus(name = "Plugin")]
    Plugin(Plugin),
}

#[derive(knus::Decode, Debug)]
struct Route {
    #[knus(argument)]
    path: String,
    #[knus(children)]
    subroutes: Vec<Route>,
}

#[derive(knus::Decode, Debug)]
struct Plugin {
    #[knus(argument)]
    name: String,
    #[knus(property)]
    url: String,
}
#[derive(knus::Decode, Debug)]
struct Call {
    #[knus(argument)]
    func_name: String,
}

#[derive(knus::Decode, Debug)]
struct Raw {
    #[knus(argument)]
    replacement: String,
}

//  #[knus(property(name="pluginName"))]

#[derive(knus::Decode, Debug)]
enum Text {
    Raw(Raw),
    Call(Call),
}

#[derive(knus::Decode, Debug)]
struct TextReplacement {
    #[knus(child)]
    with: Text,
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;

    #[test]
    // fn test_example() -> Result<(), impl Error> {
    fn test_example() {
        let config = knus::parse::<Vec<TopLevelNode>>(
            "example.kdl",
            r#"
        route "/api" {
            route "/api/v1"
        }
        plugin "http" url="https://example.org/http"
    "#,
        );

        let config = match config {
            Ok(config) => {
                println!("{config:#?}");
                config;
            }
            Err(e) => {
                println!("{:?}", &e);
                println!("{:?}", miette::Report::new(e));

                assert!(false);
            }
        };

        //  assert!(false);
    }

    // #[test]
    // // fn test_example() -> Result<(), impl Error> {
    // fn test_enum_node() {
    //     let config = knus::parse::<Vec<Text>>(
    //         "example.kdl",
    //         r#"
    //     raw "some text"
    //     call "my_function"
    // "#,
    //     );

    //     let config = match config {
    //         Ok(config) => config,
    //         Err(e) => {
    //             println!("{:?}", &e);
    //             println!("{:?}", miette::Report::new(e));
    //         }
    //     };

    //     println!("{config:#?}");
    //     assert!(false);
    // }
}
