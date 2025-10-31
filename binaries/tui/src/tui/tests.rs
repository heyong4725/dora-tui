#[cfg(test)]
use crate::tui::{
    app::{AppState, DataflowInfo, DoraApp, MessageLevel, NetworkMetrics, SystemMetrics, ViewType},
    command_executor::StateUpdate as CommandStateUpdate,
    theme::ThemeConfig,
    views::{StateUpdate, View, ViewAction},
};

#[cfg(test)]
use once_cell::sync::Lazy;

#[cfg(test)]
use std::sync::{Arc, Mutex};
#[cfg(test)]
use std::time::{Duration, Instant};

#[cfg(test)]
static CONFIG_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

#[cfg(test)]
mod app_tests {
    use super::*;
    use tui_interface::{
        MockCoordinatorClient, MockPreferencesStore, MockTelemetryService, UserPreferencesSnapshot,
    };

    #[test]
    fn test_dora_app_creation() {
        let app = DoraApp::new(ViewType::Dashboard);
        assert!(matches!(app.current_view(), &ViewType::Dashboard));
        assert!(!app.should_quit());
    }

    #[test]
    fn test_view_navigation() {
        let mut app = DoraApp::new(ViewType::Dashboard);

        // Test push view
        app.push_view(ViewType::DataflowManager);
        assert!(matches!(app.current_view(), &ViewType::DataflowManager));
        assert_eq!(app.view_stack_len(), 1);

        // Test push another view
        app.push_view(ViewType::SystemMonitor);
        assert!(matches!(app.current_view(), &ViewType::SystemMonitor));
        assert_eq!(app.view_stack_len(), 2);

        // Test pop view
        app.pop_view();
        assert!(matches!(app.current_view(), &ViewType::DataflowManager));
        assert_eq!(app.view_stack_len(), 1);

        // Test pop to original
        app.pop_view();
        assert!(matches!(app.current_view(), &ViewType::Dashboard));
        assert_eq!(app.view_stack_len(), 0);

        // Test pop when empty (should not crash)
        app.pop_view();
        assert!(matches!(app.current_view(), &ViewType::Dashboard));
        assert_eq!(app.view_stack_len(), 0);
    }

    #[test]
    fn test_view_switching() {
        let mut app = DoraApp::new(ViewType::Dashboard);

        app.switch_view(ViewType::Help);
        assert!(matches!(app.current_view(), &ViewType::Help));

        app.switch_view(ViewType::LogViewer {
            target: "test".to_string(),
        });
        assert!(matches!(app.current_view(), &ViewType::LogViewer { .. }));
    }

    #[test]
    fn test_status_messages() {
        let mut app = DoraApp::new(ViewType::Dashboard);

        app.show_status_message("Test message".to_string(), MessageLevel::Info);
        assert!(app.has_status_messages());

        app.show_error_message("Error message".to_string());
        assert!(app.has_status_messages());
    }

    #[test]
    fn test_user_preferences_reload() {
        let _lock = CONFIG_LOCK.lock().unwrap();

        let prefs_store = Arc::new(MockPreferencesStore::new());
        let coordinator = Arc::new(MockCoordinatorClient::new());
        let telemetry = Arc::new(MockTelemetryService::new());
        prefs_store.set_load_result(Ok(UserPreferencesSnapshot {
            theme: "light".to_string(),
            auto_refresh_interval_secs: 7,
            show_system_info: false,
            default_view: None,
        }));

        let mut app = DoraApp::with_dependencies(
            ViewType::Dashboard,
            prefs_store.clone(),
            coordinator.clone(),
            telemetry.clone(),
        );
        assert_eq!(app.user_config().theme_name, "light");
        assert_eq!(
            app.user_config().auto_refresh_interval,
            Duration::from_secs(7)
        );
        assert!(!app.user_config().show_system_info);

        prefs_store.set_load_result(Ok(UserPreferencesSnapshot {
            theme: "dark".to_string(),
            auto_refresh_interval_secs: 3,
            show_system_info: true,
            default_view: None,
        }));

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            app.test_process_state_update(CommandStateUpdate::ConfigurationChanged)
                .await
                .unwrap();
        });

        assert_eq!(app.user_config().theme_name, "dark");
        assert_eq!(
            app.user_config().auto_refresh_interval,
            Duration::from_secs(3)
        );
        assert!(app.user_config().show_system_info);
    }

    #[test]
    fn test_dataflow_refresh_timestamp_updates() {
        let mut app = DoraApp::new(ViewType::Dashboard);
        assert!(app.last_dataflow_refresh().is_none());

        let info = DataflowInfo {
            id: "df-test".to_string(),
            name: "demo".to_string(),
            status: "running".to_string(),
            ..Default::default()
        };

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            app.test_process_state_update(CommandStateUpdate::DataflowAdded(info))
                .await
                .unwrap();
        });

        assert!(app.last_dataflow_refresh().is_some());
    }

    #[test]
    fn test_system_metrics_history_limit() {
        let mut state = AppState::default();

        for i in 0..(AppState::system_history_capacity() + 25) {
            let network = NetworkMetrics {
                received_per_second: (i * 10) as f64,
                transmitted_per_second: (i * 5) as f64,
                ..Default::default()
            };
            let metrics = SystemMetrics {
                cpu_usage: i as f32,
                memory_usage: (i * 2) as f32,
                network,
                last_update: Some(Instant::now()),
                ..Default::default()
            };
            state.record_system_metrics(&metrics);
        }

        assert!(state.system_metrics_history().len() <= AppState::system_history_capacity());
    }
}

#[cfg(test)]
mod theme_tests {
    use super::*;

    #[test]
    fn test_theme_creation() {
        let theme = ThemeConfig::default_dark();
        assert_eq!(theme.name, "dark");

        let theme = ThemeConfig::default_light();
        assert_eq!(theme.name, "light");
    }

    #[test]
    fn test_theme_loading() {
        let theme = ThemeConfig::load_user_theme();
        assert!(!theme.name.is_empty());
    }

    #[test]
    fn test_status_styling() {
        let theme = ThemeConfig::default_dark();

        // Test status-based colors
        let running_style = theme.status_style("running");
        let error_style = theme.status_style("error");
        let warning_style = theme.status_style("warning");

        // Colors should be different for different statuses
        assert_ne!(running_style, error_style);
        assert_ne!(error_style, warning_style);
    }

    #[test]
    fn test_percentage_styling() {
        let theme = ThemeConfig::default_dark();

        let low_style = theme.percentage_style(25.0);
        let high_style = theme.percentage_style(95.0);

        // High percentage should have different (warning) color
        assert_ne!(low_style, high_style);
    }
}

#[cfg(test)]
mod view_tests {
    use super::*;
    use crate::tui::views::{DashboardView, HelpView};

    #[test]
    fn test_dashboard_view() {
        let theme = ThemeConfig::default_dark();
        let view = DashboardView::new(&theme);

        assert_eq!(view.title(), "Dashboard");
        assert!(view.can_focus());

        let help_text = view.help_text();
        assert!(!help_text.is_empty());
    }

    #[test]
    fn test_help_view() {
        let theme = ThemeConfig::default_dark();
        let view = HelpView::new(&theme);

        assert_eq!(view.title(), "Help");

        let help_text = view.help_text();
        assert!(!help_text.is_empty());
        assert!(help_text.iter().any(|(key, _)| *key == "Esc"));
    }

    #[test]
    fn test_view_actions() {
        // Test ViewAction variants
        let action = ViewAction::SwitchView(ViewType::Dashboard);
        assert!(matches!(action, ViewAction::SwitchView(_)));

        let action = ViewAction::ExecuteCommand("ps".to_string());
        assert!(matches!(action, ViewAction::ExecuteCommand(_)));

        let action = ViewAction::ShowStatus("Test".to_string());
        assert!(matches!(action, ViewAction::ShowStatus(_)));
    }

    #[test]
    fn test_state_updates() {
        let update = StateUpdate::RefreshDataflows;
        assert!(matches!(update, StateUpdate::RefreshDataflows));

        let update = StateUpdate::UpdateSystemMetrics;
        assert!(matches!(update, StateUpdate::UpdateSystemMetrics));

        let update = StateUpdate::AddStatusMessage("Test".to_string(), MessageLevel::Info);
        assert!(matches!(update, StateUpdate::AddStatusMessage(_, _)));
    }
}

#[cfg(test)]
mod app_state_tests {
    use super::*;

    #[test]
    fn test_app_state_creation() {
        let state = AppState::default();
        assert!(state.dataflows.is_empty());
        assert!(state.status_messages.is_empty());
        assert!(state.last_error.is_none());
    }

    #[test]
    fn test_message_levels() {
        let info = MessageLevel::Info;
        let success = MessageLevel::Success;
        let warning = MessageLevel::Warning;
        let error = MessageLevel::Error;

        // Test that message levels can be cloned
        let _info_clone = info.clone();
        let _success_clone = success.clone();
        let _warning_clone = warning.clone();
        let _error_clone = error.clone();
    }
}

// Tests rely on the test helper methods in DoraApp implementation
