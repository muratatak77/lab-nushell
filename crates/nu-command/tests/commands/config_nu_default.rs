use nu_test_support::prelude::*;

#[test]
fn print_config_nu_default_to_stdout() -> Result {
    test()
        .run("config nu --default")
        .expect_value_eq(nu_config::ConfigFileKind::Config.default())
}

/// Every `$env.config.history` option should have an assignment example in `config nu --doc`.
#[test]
fn config_nu_doc_documents_every_history_option() -> Result {
    test()
        .run(
            r#"
            let doc = config nu --doc
            $env.config.history
            | columns
            | where {|option| not ($doc | str contains $"$env.config.history.($option) =") }
            "#,
        )
        .expect_value_eq(Vec::<String>::new())
}
