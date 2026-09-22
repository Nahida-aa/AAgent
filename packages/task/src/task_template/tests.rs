use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use collections::{HashMap, HashSet};

use crate::{TaskContext, TaskVariables, VariableName};

use super::*;

const TEST_ID_BASE: &str = "test_base";

#[test]
fn test_resolving_templates_with_blank_command_and_label() {
    let task_with_all_properties = TaskTemplate {
        label: "test_label".to_string(),
        command: "test_command".to_string(),
        args: vec!["test_arg".to_string()],
        env: HashMap::from_iter([("test_env_key".to_string(), "test_env_var".to_string())]),
        ..TaskTemplate::default()
    };

    for task_with_blank_property in &[
        TaskTemplate {
            label: "".to_string(),
            ..task_with_all_properties.clone()
        },
        TaskTemplate {
            command: "".to_string(),
            ..task_with_all_properties.clone()
        },
        TaskTemplate {
            label: "".to_string(),
            command: "".to_string(),
            ..task_with_all_properties
        },
    ] {
        assert_eq!(
            task_with_blank_property.resolve_task(TEST_ID_BASE, &TaskContext::default()),
            None,
            "should not resolve task with blank label and/or command: {task_with_blank_property:?}"
        );
    }
}

#[test]
fn test_template_cwd_resolution() {
    let task_without_cwd = TaskTemplate {
        cwd: None,
        label: "test task".to_string(),
        command: "echo 4".to_string(),
        ..TaskTemplate::default()
    };

    let resolved_task = |task_template: &TaskTemplate, task_cx| {
        let resolved_task = task_template
            .resolve_task(TEST_ID_BASE, task_cx)
            .unwrap_or_else(|| panic!("failed to resolve task {task_without_cwd:?}"));
        assert_substituted_variables(&resolved_task, Vec::new());
        resolved_task.resolved
    };

    let cx = TaskContext {
        cwd: None,
        task_variables: TaskVariables::default(),
        project_env: HashMap::default(),
    };
    assert_eq!(
        resolved_task(&task_without_cwd, &cx).cwd,
        None,
        "When neither task nor task context have cwd, it should be None"
    );

    let context_cwd = Path::new("a").join("b").join("c");
    let cx = TaskContext {
        cwd: Some(context_cwd.clone()),
        task_variables: TaskVariables::default(),
        project_env: HashMap::default(),
    };
    assert_eq!(
        resolved_task(&task_without_cwd, &cx).cwd,
        Some(context_cwd.clone()),
        "TaskContext's cwd should be taken on resolve if task's cwd is None"
    );

    let task_cwd = Path::new("d").join("e").join("f");
    let mut task_with_cwd = task_without_cwd.clone();
    task_with_cwd.cwd = Some(task_cwd.display().to_string());
    let task_with_cwd = task_with_cwd;

    let cx = TaskContext {
        cwd: None,
        task_variables: TaskVariables::default(),
        project_env: HashMap::default(),
    };
    assert_eq!(
        resolved_task(&task_with_cwd, &cx).cwd,
        Some(task_cwd.clone()),
        "TaskTemplate's cwd should be taken on resolve if TaskContext's cwd is None"
    );

    let cx = TaskContext {
        cwd: Some(context_cwd),
        task_variables: TaskVariables::default(),
        project_env: HashMap::default(),
    };
    assert_eq!(
        resolved_task(&task_with_cwd, &cx).cwd,
        Some(task_cwd),
        "TaskTemplate's cwd should be taken on resolve if TaskContext's cwd is not None"
    );
}

#[test]
fn test_template_variables_resolution() {
    let custom_variable_1 = VariableName::Custom(Cow::Borrowed("custom_variable_1"));
    let custom_variable_2 = VariableName::Custom(Cow::Borrowed("custom_variable_2"));
    let long_value = "01".repeat(MAX_DISPLAY_VARIABLE_LENGTH * 2);
    let all_variables = [
        (VariableName::Row, "1234".to_string()),
        (VariableName::Column, "5678".to_string()),
        (VariableName::File, "test_file".to_string()),
        (VariableName::SelectedText, "test_selected_text".to_string()),
        (VariableName::Symbol, long_value.clone()),
        (VariableName::WorktreeRoot, "/test_root/".to_string()),
        (
            custom_variable_1.clone(),
            "test_custom_variable_1".to_string(),
        ),
        (
            custom_variable_2.clone(),
            "test_custom_variable_2".to_string(),
        ),
    ];

    let task_with_all_variables = TaskTemplate {
        label: format!(
            "test label for {} and {}",
            VariableName::Row.template_value(),
            VariableName::Symbol.template_value(),
        ),
        command: format!(
            "echo {} {}",
            VariableName::File.template_value(),
            VariableName::Symbol.template_value(),
        ),
        args: vec![
            format!("arg1 {}", VariableName::SelectedText.template_value()),
            format!("arg2 {}", VariableName::Column.template_value()),
            format!("arg3 {}", VariableName::Symbol.template_value()),
        ],
        env: HashMap::from_iter([
            ("test_env_key".to_string(), "test_env_var".to_string()),
            (
                "env_key_1".to_string(),
                VariableName::WorktreeRoot.template_value(),
            ),
            (
                "env_key_2".to_string(),
                format!(
                    "env_var_2 {} {}",
                    custom_variable_1.template_value(),
                    custom_variable_2.template_value()
                ),
            ),
            (
                "env_key_3".to_string(),
                format!("env_var_3 {}", VariableName::Symbol.template_value()),
            ),
        ]),
        ..TaskTemplate::default()
    };

    let mut first_resolved_id = None;
    for i in 0..15 {
        let resolved_task = task_with_all_variables.resolve_task(
            TEST_ID_BASE,
            &TaskContext {
                cwd: None,
                task_variables: TaskVariables::from_iter(all_variables.clone()),
                project_env: HashMap::default(),
            },
        ).unwrap_or_else(|| panic!("Should successfully resolve task {task_with_all_variables:?} with variables {all_variables:?}"));

        match &first_resolved_id {
            None => first_resolved_id = Some(resolved_task.id.clone()),
            Some(first_id) => assert_eq!(
                &resolved_task.id, first_id,
                "Step {i}, for the same task template and context, there should be the same resolved task id"
            ),
        }

        assert_eq!(
            resolved_task.original_task, task_with_all_variables,
            "Resolved task should store its template without changes"
        );
        assert_eq!(
            resolved_task.resolved_label,
            format!("test label for 1234 and {long_value}"),
            "Resolved task label should be substituted with variables and those should not be shortened"
        );
        assert_substituted_variables(
            &resolved_task,
            all_variables.iter().map(|(name, _)| name.clone()).collect(),
        );

        let spawn_in_terminal = &resolved_task.resolved;
        assert_eq!(
            spawn_in_terminal.label,
            format!(
                "test label for 1234 and …{}",
                &long_value[long_value.len() - MAX_DISPLAY_VARIABLE_LENGTH..]
            ),
            "Human-readable label should have long substitutions trimmed"
        );
        assert_eq!(
            spawn_in_terminal.command.clone().unwrap(),
            format!("echo test_file {long_value}"),
            "Command should be substituted with variables and those should not be shortened"
        );
        assert_eq!(
            spawn_in_terminal.args,
            &[
                "arg1 test_selected_text",
                "arg2 5678",
                "arg3 010101010101010101010101010101010101010101010101010101010101",
            ],
            "Args should be substituted with variables"
        );
        assert_eq!(
            spawn_in_terminal.command_label,
            format!(
                "{} arg1 test_selected_text arg2 5678 arg3 {long_value}",
                spawn_in_terminal.command.clone().unwrap()
            ),
            "Command label args should be substituted with variables and those should not be shortened"
        );

        assert_eq!(
            spawn_in_terminal
                .env
                .get("test_env_key")
                .map(|s| s.as_str()),
            Some("test_env_var")
        );
        assert_eq!(
            spawn_in_terminal.env.get("env_key_1").map(|s| s.as_str()),
            Some("/test_root/")
        );
        assert_eq!(
            spawn_in_terminal.env.get("env_key_2").map(|s| s.as_str()),
            Some("env_var_2 test_custom_variable_1 test_custom_variable_2")
        );
        assert_eq!(
            spawn_in_terminal.env.get("env_key_3"),
            Some(&format!("env_var_3 {long_value}")),
            "Env vars should be substituted with variables and those should not be shortened"
        );
    }

    for i in 0..all_variables.len() {
        let mut not_all_variables = all_variables.to_vec();
        let removed_variable = not_all_variables.remove(i);
        let resolved_task_attempt = task_with_all_variables.resolve_task(
            TEST_ID_BASE,
            &TaskContext {
                cwd: None,
                task_variables: TaskVariables::from_iter(not_all_variables),
                project_env: HashMap::default(),
            },
        );
        assert!(
            matches!(resolved_task_attempt, None),
            "If any of the Zed task variables is not substituted, the task should not be resolved, but got some resolution without the variable {removed_variable:?} (index {i})"
        );
    }
}

#[test]
fn test_can_resolve_free_variables() {
    let task = TaskTemplate {
        label: "My task".into(),
        command: "echo".into(),
        args: vec!["$PATH".into()],
        ..TaskTemplate::default()
    };
    let resolved_task = task
        .resolve_task(TEST_ID_BASE, &TaskContext::default())
        .unwrap();
    assert_substituted_variables(&resolved_task, Vec::new());
    let resolved = resolved_task.resolved;
    assert_eq!(resolved.label, task.label);
    assert_eq!(resolved.command, Some(task.command));
    assert_eq!(resolved.args, task.args);
}

#[test]
fn test_errors_on_missing_zed_variable() {
    let task = TaskTemplate {
        label: "My task".into(),
        command: "echo".into(),
        args: vec!["$ZED_VARIABLE".into()],
        ..TaskTemplate::default()
    };
    assert!(
        task.resolve_task(TEST_ID_BASE, &TaskContext::default())
            .is_none()
    );
}

#[test]
fn test_symbol_dependent_tasks() {
    let task_with_all_properties = TaskTemplate {
        label: "test_label".to_string(),
        command: "test_command".to_string(),
        args: vec!["test_arg".to_string()],
        env: HashMap::from_iter([("test_env_key".to_string(), "test_env_var".to_string())]),
        ..TaskTemplate::default()
    };
    let cx = TaskContext {
        cwd: None,
        task_variables: TaskVariables::from_iter(Some((
            VariableName::Symbol,
            "test_symbol".to_string(),
        ))),
        project_env: HashMap::default(),
    };

    for (i, symbol_dependent_task) in [
        TaskTemplate {
            label: format!("test_label_{}", VariableName::Symbol.template_value()),
            ..task_with_all_properties.clone()
        },
        TaskTemplate {
            command: format!("test_command_{}", VariableName::Symbol.template_value()),
            ..task_with_all_properties.clone()
        },
        TaskTemplate {
            args: vec![format!(
                "test_arg_{}",
                VariableName::Symbol.template_value()
            )],
            ..task_with_all_properties.clone()
        },
        TaskTemplate {
            env: HashMap::from_iter([(
                "test_env_key".to_string(),
                format!("test_env_var_{}", VariableName::Symbol.template_value()),
            )]),
            ..task_with_all_properties
        },
    ]
    .into_iter()
    .enumerate()
    {
        let resolved = symbol_dependent_task
            .resolve_task(TEST_ID_BASE, &cx)
            .unwrap_or_else(|| panic!("Failed to resolve task {symbol_dependent_task:?}"));
        assert_eq!(
            resolved.substituted_variables,
            HashSet::from_iter(Some(VariableName::Symbol)),
            "(index {i}) Expected the task to depend on symbol task variable: {resolved:?}"
        )
    }
}

#[track_caller]
fn assert_substituted_variables(resolved_task: &ResolvedTask, mut expected: Vec<VariableName>) {
    let mut resolved_variables = resolved_task
        .substituted_variables
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    resolved_variables.sort_by_key(|var| var.to_string());
    expected.sort_by_key(|var| var.to_string());
    assert_eq!(resolved_variables, expected)
}

#[test]
fn substitute_funky_labels() {
    let faulty_go_test = TaskTemplate {
        label: format!(
            "go test {}/{}",
            VariableName::Symbol.template_value(),
            VariableName::Symbol.template_value(),
        ),
        command: "go".into(),
        args: vec![format!(
            "^{}$/^{}$",
            VariableName::Symbol.template_value(),
            VariableName::Symbol.template_value()
        )],
        ..TaskTemplate::default()
    };
    let mut context = TaskContext::default();
    context
        .task_variables
        .insert(VariableName::Symbol, "my-symbol".to_string());
    assert!(faulty_go_test.resolve_task("base", &context).is_some());
}

#[test]
fn test_project_env() {
    let all_variables = [
        (VariableName::Row, "1234".to_string()),
        (VariableName::Column, "5678".to_string()),
        (VariableName::File, "test_file".to_string()),
        (VariableName::Symbol, "my symbol".to_string()),
    ];

    let template = TaskTemplate {
        label: "my task".to_string(),
        command: format!(
            "echo {} {}",
            VariableName::File.template_value(),
            VariableName::Symbol.template_value(),
        ),
        args: vec![],
        env: HashMap::from_iter([
            (
                "TASK_ENV_VAR1".to_string(),
                "TASK_ENV_VAR1_VALUE".to_string(),
            ),
            (
                "TASK_ENV_VAR2".to_string(),
                format!(
                    "env_var_2 {} {}",
                    VariableName::Row.template_value(),
                    VariableName::Column.template_value()
                ),
            ),
            (
                "PROJECT_ENV_WILL_BE_OVERWRITTEN".to_string(),
                "overwritten".to_string(),
            ),
        ]),
        ..TaskTemplate::default()
    };

    let project_env = HashMap::from_iter([
        (
            "PROJECT_ENV_VAR1".to_string(),
            "PROJECT_ENV_VAR1_VALUE".to_string(),
        ),
        (
            "PROJECT_ENV_WILL_BE_OVERWRITTEN".to_string(),
            "PROJECT_ENV_WILL_BE_OVERWRITTEN_VALUE".to_string(),
        ),
    ]);

    let context = TaskContext {
        cwd: None,
        task_variables: TaskVariables::from_iter(all_variables),
        project_env,
    };

    let resolved = template
        .resolve_task(TEST_ID_BASE, &context)
        .unwrap()
        .resolved;

    assert_eq!(resolved.env["TASK_ENV_VAR1"], "TASK_ENV_VAR1_VALUE");
    assert_eq!(resolved.env["TASK_ENV_VAR2"], "env_var_2 1234 5678");
    assert_eq!(resolved.env["PROJECT_ENV_VAR1"], "PROJECT_ENV_VAR1_VALUE");
    assert_eq!(
        resolved.env["PROJECT_ENV_WILL_BE_OVERWRITTEN"],
        "overwritten"
    );
}

#[test]
fn test_variable_default_values() {
    let task_with_defaults = TaskTemplate {
        label: "test with defaults".to_string(),
        command: format!(
            "echo ${{{}}}",
            VariableName::File.to_string() + ":fallback.txt"
        ),
        args: vec![
            "${ZED_MISSING_VAR:default_value}".to_string(),
            format!("${{{}}}", VariableName::Row.to_string() + ":42"),
        ],
        ..TaskTemplate::default()
    };

    // Test 1: When ZED_FILE exists, should use actual value and ignore default
    let context_with_file = TaskContext {
        cwd: None,
        task_variables: TaskVariables::from_iter(vec![
            (VariableName::File, "actual_file.rs".to_string()),
            (VariableName::Row, "123".to_string()),
        ]),
        project_env: HashMap::default(),
    };

    let resolved = task_with_defaults
        .resolve_task(TEST_ID_BASE, &context_with_file)
        .expect("Should resolve task with existing variables");

    assert_eq!(
        resolved.resolved.command.unwrap(),
        "echo actual_file.rs",
        "Should use actual ZED_FILE value, not default"
    );
    assert_eq!(
        resolved.resolved.args,
        vec!["default_value", "123"],
        "Should use default for missing var, actual value for existing var"
    );

    // Test 2: When ZED_FILE doesn't exist, should use default value
    let context_without_file = TaskContext {
        cwd: None,
        task_variables: TaskVariables::from_iter(vec![(VariableName::Row, "456".to_string())]),
        project_env: HashMap::default(),
    };

    let resolved = task_with_defaults
        .resolve_task(TEST_ID_BASE, &context_without_file)
        .expect("Should resolve task using default values");

    assert_eq!(
        resolved.resolved.command.unwrap(),
        "echo fallback.txt",
        "Should use default value when ZED_FILE is missing"
    );
    assert_eq!(
        resolved.resolved.args,
        vec!["default_value", "456"],
        "Should use defaults for missing vars"
    );

    // Test 3: Missing ZED variable without default should fail
    let task_no_default = TaskTemplate {
        label: "test no default".to_string(),
        command: "${ZED_MISSING_NO_DEFAULT}".to_string(),
        ..TaskTemplate::default()
    };

    assert!(
        task_no_default
            .resolve_task(TEST_ID_BASE, &TaskContext::default())
            .is_none(),
        "Should fail when ZED variable has no default and doesn't exist"
    );
}

#[test]
fn test_unknown_variables() {
    // Variable names starting with `ZED_` that are not valid should be
    // reported.
    let label = "test unknown variables".to_string();
    let command = "$ZED_UNKNOWN".to_string();
    let task = TaskTemplate {
        label,
        command,
        ..TaskTemplate::default()
    };

    assert_eq!(task.unknown_variables(), vec!["ZED_UNKNOWN".to_string()]);

    // Variable names starting with `ZED_CUSTOM_` should never be reported,
    // as those are dynamically provided by extensions.
    let label = "test custom variables".to_string();
    let command = "$ZED_CUSTOM_UNKNOWN".to_string();
    let task = TaskTemplate {
        label,
        command,
        ..TaskTemplate::default()
    };

    assert!(task.unknown_variables().is_empty());

    // Unknown variable names with defaults should still be reported,
    // otherwise the default would always be silently used.
    let label = "test custom variables".to_string();
    let command = "${ZED_UNKNOWN:default_value}".to_string();
    let task = TaskTemplate {
        label,
        command,
        ..TaskTemplate::default()
    };

    assert_eq!(task.unknown_variables(), vec!["ZED_UNKNOWN".to_string()]);

    // Valid variable names are not reported.
    let label = "test custom variables".to_string();
    let command = "$ZED_FILE".to_string();
    let task = TaskTemplate {
        label,
        command,
        ..TaskTemplate::default()
    };
    assert!(task.unknown_variables().is_empty());
}

#[test]
fn test_git_variables_resolution() {
    let task = TaskTemplate {
        label: "Show $ZED_GIT_SHA_SHORT in $ZED_GIT_REPOSITORY_NAME".to_string(),
        command: "git".to_string(),
        args: vec!["show".to_string(), "$ZED_GIT_SHA".to_string()],
        cwd: Some("$ZED_GIT_REPOSITORY_PATH".to_string()),
        env: HashMap::from_iter([("COMMIT".to_string(), "$ZED_GIT_SHA".to_string())]),
        ..TaskTemplate::default()
    };
    let sha = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string();
    let sha_short = "0123456".to_string();
    let repo_name = "zed".to_string();
    let repo_path = format!("/Users/example/{repo_name}");

    let context = TaskContext {
        task_variables: TaskVariables::from_iter([
            (VariableName::GitSha, sha.clone()),
            (VariableName::GitShaShort, sha_short.clone()),
            (VariableName::GitRepositoryName, repo_name.clone()),
            (VariableName::GitRepositoryPath, repo_path.clone()),
        ]),
        ..TaskContext::default()
    };

    let task = task.resolve_task(TEST_ID_BASE, &context).unwrap();
    assert_eq!(
        task.resolved_label,
        format!("Show {sha_short} in {repo_name}")
    );
    assert_eq!(task.resolved.command, Some("git".to_string()));
    assert_eq!(task.resolved.args, vec!["show".to_string(), sha.clone()]);
    assert_eq!(task.resolved.cwd, Some(PathBuf::from(repo_path)));
    assert_eq!(task.resolved.env.get("COMMIT"), Some(&sha));

    assert_substituted_variables(
        &task,
        vec![
            VariableName::GitSha,
            VariableName::GitShaShort,
            VariableName::GitRepositoryName,
            VariableName::GitRepositoryPath,
        ],
    );
}

#[test]
fn test_args_produced_by_empty_variables_are_omitted() {
    let features_flag = VariableName::Custom(Cow::Borrowed("features_flag"));
    let features = VariableName::Custom(Cow::Borrowed("features"));
    let bin_name = VariableName::Custom(Cow::Borrowed("bin_name"));

    let task = TaskTemplate {
        label: "cargo run".to_string(),
        command: "cargo".to_string(),
        args: vec![
            "run".to_string(),
            "--bin".to_string(),
            bin_name.template_value(),
            features_flag.template_value(),
            features.template_value(),
            String::new(),
            format!("--config={}", features.template_value()),
        ],
        ..TaskTemplate::default()
    };

    let context = TaskContext {
        task_variables: TaskVariables::from_iter([
            (features_flag, String::new()),
            (features, String::new()),
            (bin_name, "test_bin".to_string()),
        ]),
        ..TaskContext::default()
    };

    let resolved = task
        .resolve_task(TEST_ID_BASE, &context)
        .unwrap_or_else(|| panic!("failed to resolve task {task:?}"))
        .resolved;
    assert_eq!(
        resolved.args,
        vec!["run", "--bin", "test_bin", "", "--config="],
        "args that consist entirely of variables resolved to empty strings should be omitted, \
        while literal empty args and partially substituted args should be preserved"
    );
}
