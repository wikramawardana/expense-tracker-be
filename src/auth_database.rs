const ALLOWED_AUTH_DATABASES: [&str; 2] = ["expense_tracker_auth", "expense_tracker_auth_dev"];

pub fn require_isolated_auth_database(database_url: &str, app_name: &str) -> Result<(), String> {
    let without_query = database_url
        .split_once('?')
        .map_or(database_url, |(value, _)| value);
    let without_fragment = without_query
        .split_once('#')
        .map_or(without_query, |(value, _)| value);
    let database_name = without_fragment
        .rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "DATABASE_URL must include a database name".to_string())?;

    if !ALLOWED_AUTH_DATABASES.contains(&database_name) {
        return Err(format!(
            "{app_name} DATABASE_URL must point to its dedicated auth database"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::require_isolated_auth_database;

    #[test]
    fn accepts_expense_tracker_database() {
        assert!(require_isolated_auth_database(
            "postgresql://user:password@postgres:5432/expense_tracker_auth",
            "Expense Tracker API",
        )
        .is_ok());
    }

    #[test]
    fn rejects_central_database() {
        assert!(require_isolated_auth_database(
            "postgresql://user:password@postgres:5432/auth?sslmode=require",
            "Expense Tracker API",
        )
        .is_err());
    }

    #[test]
    fn rejects_another_apps_database() {
        assert!(require_isolated_auth_database(
            "postgresql://user:password@postgres:5432/tuwaga_auth",
            "Expense Tracker API",
        )
        .is_err());
    }
}
