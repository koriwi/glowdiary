use std::sync::{Arc, Mutex};

use rmcp::{
    ErrorData as McpError,
    ServerHandler,
    handler::server::router::tool::ToolRouter,
    handler::server::tool::ToolCallContext,
    handler::server::wrapper::Parameters,
    model::{
        CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams,
        ServerCapabilities, ServerInfo, Tool,
    },
    service::{RequestContext, RoleServer},
    schemars, tool, tool_router,
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db;
use crate::off;

// ---------------------------------------------------------------------------
// Core server state
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct GlowDiary {
    db: Arc<Mutex<Connection>>,
    tool_router: ToolRouter<Self>,
}

impl GlowDiary {
    pub fn new(conn: Connection) -> Self {
        Self {
            db: Arc::new(Mutex::new(conn)),
            tool_router: Self::tool_router(),
        }
    }

    /// Run a closure with the database locked, on a blocking thread.
    fn with_db<T, F>(&self, f: F) -> String
    where
        T: Serialize,
        F: FnOnce(&Connection) -> Result<T, crate::error::AppError> + Send + 'static,
    {
        let db = self.db.clone();
        tokio::task::block_in_place(move || {
            let conn = db.lock().unwrap();
            match f(&conn) {
                Ok(value) => serde_json::to_string_pretty(&value).unwrap_or_else(|e| {
                    format!("{{\"error\": \"Serialization failed: {e}\"}}")
                }),
                Err(e) => format!("{{\"error\": \"{e}\"}}"),
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Parameter types for each tool
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct NameParam {
    #[schemars(description = "User's display name")]
    pub name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UserUuidParam {
    #[schemars(description = "User UUID returned by register_user")]
    pub user_uuid: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetGoalsParams {
    #[schemars(description = "User UUID")]
    pub user_uuid: String,
    #[schemars(description = "Daily calorie target in kcal")]
    pub kcal_target: f64,
    #[schemars(description = "Daily fat target in grams")]
    pub fat_g_target: f64,
    #[schemars(description = "Daily protein target in grams")]
    pub protein_g_target: f64,
    #[schemars(description = "Daily carbohydrate target in grams")]
    pub carbs_g_target: f64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddMealParams {
    #[schemars(description = "User UUID")]
    pub user_uuid: String,
    #[schemars(description = "Meal/food name")]
    pub name: String,
    #[schemars(description = "ISO 8601 datetime when the food was eaten (e.g. '2026-05-13T12:30:00')")]
    pub eaten_at: String,
    #[schemars(description = "Energy in kilocalories")]
    pub kcal: f64,
    #[schemars(description = "Fat in grams")]
    pub fat_g: f64,
    #[schemars(description = "Protein in grams")]
    pub protein_g: f64,
    #[schemars(description = "Carbohydrates in grams")]
    pub carbs_g: f64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddMealFromFoodParams {
    #[schemars(description = "User UUID")]
    pub user_uuid: String,
    #[schemars(description = "Meal/food name")]
    pub name: String,
    #[schemars(description = "ISO 8601 datetime when the food was eaten (e.g. '2026-05-13T12:30:00')")]
    pub eaten_at: String,
    #[schemars(description = "Barcode from a search_food or lookup_barcode result")]
    pub barcode: String,
    #[schemars(description = "Weight of the food in grams")]
    pub grams: f64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MealUuidParam {
    #[schemars(description = "Meal UUID")]
    pub uuid: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DateParam {
    #[schemars(description = "User UUID")]
    pub user_uuid: String,
    #[schemars(description = "ISO date string (e.g. '2026-05-13')")]
    pub date: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchFoodParams {
    #[schemars(description = "Food product name to search for")]
    pub query: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BarcodeParam {
    #[schemars(description = "Product barcode / GTIN")]
    pub barcode: String,
}

// ---------------------------------------------------------------------------
// Response types for stats tools
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct StatsWithGoals {
    date: String,
    total_kcal: f64,
    total_fat_g: f64,
    total_protein_g: f64,
    total_carbs_g: f64,
    meal_count: u64,
    goals: db::goals::Goals,
    remaining_kcal: f64,
    remaining_fat_g: f64,
    remaining_protein_g: f64,
    remaining_carbs_g: f64,
    kcal_percent: f64,
    fat_percent: f64,
    protein_percent: f64,
    carbs_percent: f64,
}

#[derive(Serialize)]
struct WeeklyWithGoals {
    week_start: String,
    week_end: String,
    total_kcal: f64,
    total_fat_g: f64,
    total_protein_g: f64,
    total_carbs_g: f64,
    total_meal_count: u64,
    daily_average_kcal: f64,
    daily_average_fat_g: f64,
    daily_average_protein_g: f64,
    daily_average_carbs_g: f64,
    daily_average_kcal_percent: f64,
    per_day: Vec<db::meals::DailyStats>,
    goals: db::goals::Goals,
}

// ---------------------------------------------------------------------------
// Tool implementations
// ---------------------------------------------------------------------------

#[tool_router]
impl GlowDiary {
    // -----------------------------------------------------------------------
    // User management
    // -----------------------------------------------------------------------

    #[tool(description = "Register a new user. Creates a user with default daily goals (2000 kcal, 65g fat, 75g protein, 275g carbs). Returns the user UUID which must be used for all subsequent calls.")]
    fn register_user(
        &self,
        Parameters(NameParam { name }): Parameters<NameParam>,
    ) -> String {
        self.with_db(move |conn| {
            let user = db::users::create_user(conn, &name)?;
            let goals = db::goals::get_goals(conn, &user.uuid)?;
            #[derive(Serialize)]
            struct Response {
                uuid: String,
                name: String,
                goals: db::goals::Goals,
                message: String,
            }
            Ok(Response {
                uuid: user.uuid,
                name: user.name,
                goals,
                message: "User created. Use this uuid for all future calls. You can adjust goals with set_goals at any time.".to_string(),
            })
        })
    }

    #[tool(description = "Get user information by UUID. Returns the user's name and creation date.")]
    fn get_user(
        &self,
        Parameters(UserUuidParam { user_uuid }): Parameters<UserUuidParam>,
    ) -> String {
        self.with_db(move |conn| {
            let user = db::users::get_user(conn, &user_uuid)?
                .ok_or_else(|| crate::error::AppError::UserNotFound(user_uuid.clone()))?;
            Ok(user)
        })
    }

    // -----------------------------------------------------------------------
    // Goals
    // -----------------------------------------------------------------------

    #[tool(description = "Set daily nutrition goals for a user. All parameters are required.")]
    fn set_goals(
        &self,
        Parameters(SetGoalsParams {
            user_uuid,
            kcal_target,
            fat_g_target,
            protein_g_target,
            carbs_g_target,
        }): Parameters<SetGoalsParams>,
    ) -> String {
        self.with_db(move |conn| {
            db::users::require_user(conn, &user_uuid)?;
            let goals = db::goals::set_goals(
                conn,
                &user_uuid,
                kcal_target,
                fat_g_target,
                protein_g_target,
                carbs_g_target,
            )?;
            Ok(goals)
        })
    }

    #[tool(description = "Get current daily nutrition goals for a user.")]
    fn get_goals(
        &self,
        Parameters(UserUuidParam { user_uuid }): Parameters<UserUuidParam>,
    ) -> String {
        self.with_db(move |conn| {
            db::users::require_user(conn, &user_uuid)?;
            let goals = db::goals::get_goals(conn, &user_uuid)?;
            Ok(goals)
        })
    }

    // -----------------------------------------------------------------------
    // Meals — Create
    // -----------------------------------------------------------------------

    #[tool(description = "Log a meal with manually specified nutrition values (kcal, fat, protein, carbs).")]
    fn add_meal(
        &self,
        Parameters(AddMealParams {
            user_uuid,
            name,
            eaten_at,
            kcal,
            fat_g,
            protein_g,
            carbs_g,
        }): Parameters<AddMealParams>,
    ) -> String {
        self.with_db(move |conn| {
            db::users::require_user(conn, &user_uuid)?;
            let meal = db::meals::add_meal(
                conn, &user_uuid, &name, &eaten_at, kcal, fat_g, protein_g, carbs_g, None,
            )?;
            Ok(meal)
        })
    }

    #[tool(description = "Log a meal from a food product looked up via search_food or lookup_barcode. Specify the barcode and the amount eaten in grams. Nutrition is automatically calculated from the product's per-100g data.")]
    fn add_meal_from_food(
        &self,
        Parameters(params): Parameters<AddMealFromFoodParams>,
    ) -> String {
        if params.grams <= 0.0 {
            return format!("{{\"error\": \"grams must be > 0, got {}\"}}", params.grams);
        }

        let db = self.db.clone();
        tokio::task::block_in_place(move || {
            let product = match off::lookup_barcode(&params.barcode) {
                Ok(p) => p,
                Err(e) => return format!(
                    "{{\"error\": \"Failed to look up barcode '{}': {e}\"}}",
                    params.barcode
                ),
            };

            let nutrition = off::compute_nutrition(&product.per_100g, params.grams);

            let conn = db.lock().unwrap();
            match (|| -> crate::error::AppResult<_> {
                db::users::require_user(&conn, &params.user_uuid)?;
                db::meals::add_meal(
                    &conn,
                    &params.user_uuid,
                    &params.name,
                    &params.eaten_at,
                    nutrition.kcal,
                    nutrition.fat_g,
                    nutrition.protein_g,
                    nutrition.carbs_g,
                    Some(params.barcode),
                )
            })() {
                Ok(meal) => serde_json::to_string_pretty(&meal)
                    .unwrap_or_else(|e| format!("{{\"error\": \"Serialization failed: {e}\"}}")),
                Err(e) => format!("{{\"error\": \"{e}\"}}"),
            }
        })
    }

    // -----------------------------------------------------------------------
    // Meals — Read
    // -----------------------------------------------------------------------

    #[tool(description = "Get a single meal by its UUID.")]
    fn get_meal(
        &self,
        Parameters(MealUuidParam { uuid }): Parameters<MealUuidParam>,
    ) -> String {
        self.with_db(move |conn| {
            let meal = db::meals::get_meal(conn, &uuid)?;
            Ok(meal)
        })
    }

    #[tool(description = "Get all meals for a user on a specific day, sorted by eaten_at ascending.")]
    fn get_meals_by_day(
        &self,
        Parameters(DateParam { user_uuid, date }): Parameters<DateParam>,
    ) -> String {
        self.with_db(move |conn| {
            db::users::require_user(conn, &user_uuid)?;
            let meals = db::meals::get_meals_by_day(conn, &user_uuid, &date)?;
            Ok(meals)
        })
    }

    #[tool(description = "Get all meals for a user in the ISO week containing the given date, sorted by eaten_at ascending.")]
    fn get_meals_by_week(
        &self,
        Parameters(DateParam { user_uuid, date }): Parameters<DateParam>,
    ) -> String {
        self.with_db(move |conn| {
            db::users::require_user(conn, &user_uuid)?;
            let meals = db::meals::get_meals_by_week(conn, &user_uuid, &date)?;
            Ok(meals)
        })
    }

    // -----------------------------------------------------------------------
    // Meals — Delete
    // -----------------------------------------------------------------------

    #[tool(description = "Delete a meal by its UUID. Returns the deleted meal.")]
    fn delete_meal(
        &self,
        Parameters(MealUuidParam { uuid }): Parameters<MealUuidParam>,
    ) -> String {
        self.with_db(move |conn| {
            let meal = db::meals::delete_meal(conn, &uuid)?;
            #[derive(Serialize)]
            struct Response {
                deleted: db::meals::Meal,
                message: String,
            }
            Ok(Response {
                deleted: meal,
                message: "Meal deleted.".to_string(),
            })
        })
    }

    // -----------------------------------------------------------------------
    // Stats
    // -----------------------------------------------------------------------

    #[tool(description = "Get daily nutrition stats for a user: totals vs goals with remaining amounts and completion percentages.")]
    fn get_stats(
        &self,
        Parameters(DateParam { user_uuid, date }): Parameters<DateParam>,
    ) -> String {
        self.with_db(move |conn| {
            db::users::require_user(conn, &user_uuid)?;
            let stats = db::meals::get_daily_stats(conn, &user_uuid, &date)?;
            let goals = db::goals::get_goals(conn, &user_uuid)?;

            let remaining_kcal = (goals.kcal_target - stats.total_kcal).max(0.0);
            let remaining_fat_g = (goals.fat_g_target - stats.total_fat_g).max(0.0);
            let remaining_protein_g = (goals.protein_g_target - stats.total_protein_g).max(0.0);
            let remaining_carbs_g = (goals.carbs_g_target - stats.total_carbs_g).max(0.0);

            let kcal_percent = if goals.kcal_target > 0.0 {
                (stats.total_kcal / goals.kcal_target) * 100.0
            } else {
                0.0
            };
            let fat_percent = if goals.fat_g_target > 0.0 {
                (stats.total_fat_g / goals.fat_g_target) * 100.0
            } else {
                0.0
            };
            let protein_percent = if goals.protein_g_target > 0.0 {
                (stats.total_protein_g / goals.protein_g_target) * 100.0
            } else {
                0.0
            };
            let carbs_percent = if goals.carbs_g_target > 0.0 {
                (stats.total_carbs_g / goals.carbs_g_target) * 100.0
            } else {
                0.0
            };

            Ok(StatsWithGoals {
                date: stats.date,
                total_kcal: stats.total_kcal,
                total_fat_g: stats.total_fat_g,
                total_protein_g: stats.total_protein_g,
                total_carbs_g: stats.total_carbs_g,
                meal_count: stats.meal_count,
                goals,
                remaining_kcal,
                remaining_fat_g,
                remaining_protein_g,
                remaining_carbs_g,
                kcal_percent,
                fat_percent,
                protein_percent,
                carbs_percent,
            })
        })
    }

    #[tool(description = "Get weekly nutrition stats for a user: totals, daily averages, per-day breakdown. Use the date of any day in the target week.")]
    fn get_weekly_stats(
        &self,
        Parameters(DateParam { user_uuid, date }): Parameters<DateParam>,
    ) -> String {
        self.with_db(move |conn| {
            db::users::require_user(conn, &user_uuid)?;
            let stats = db::meals::get_weekly_stats(conn, &user_uuid, &date)?;
            let goals = db::goals::get_goals(conn, &user_uuid)?;

            let daily_avg_kcal_pct = if goals.kcal_target > 0.0 {
                (stats.daily_averages.total_kcal / goals.kcal_target) * 100.0
            } else {
                0.0
            };

            Ok(WeeklyWithGoals {
                week_start: stats.week_start,
                week_end: stats.week_end,
                total_kcal: stats.totals.total_kcal,
                total_fat_g: stats.totals.total_fat_g,
                total_protein_g: stats.totals.total_protein_g,
                total_carbs_g: stats.totals.total_carbs_g,
                total_meal_count: stats.totals.meal_count,
                daily_average_kcal: stats.daily_averages.total_kcal,
                daily_average_fat_g: stats.daily_averages.total_fat_g,
                daily_average_protein_g: stats.daily_averages.total_protein_g,
                daily_average_carbs_g: stats.daily_averages.total_carbs_g,
                daily_average_kcal_percent: daily_avg_kcal_pct,
                per_day: stats.per_day,
                goals,
            })
        })
    }

    // -----------------------------------------------------------------------
    // Open Food Facts
    // -----------------------------------------------------------------------

    #[tool(description = "Search for food products on Open Food Facts. Returns product name, barcode, per-100g nutrition values, and if available, serving size info with per-serving values. Use this to find foods before logging them with add_meal_from_food.")]
    fn search_food(
        &self,
        Parameters(SearchFoodParams { query }): Parameters<SearchFoodParams>,
    ) -> String {
        tokio::task::block_in_place(move || {
            match off::search(&query) {
                Ok(results) => {
                    serde_json::to_string_pretty(&serde_json::json!({ "results": results }))
                        .unwrap_or_else(|e| format!("{{\"error\": \"Serialization failed: {e}\"}}"))
                }
                Err(e) => format!("{{\"error\": \"{e}\"}}"),
            }
        })
    }

    #[tool(description = "Look up a food product by its barcode on Open Food Facts. Returns product name, barcode, per-100g nutrition, and serving size info if available.")]
    fn lookup_barcode(
        &self,
        Parameters(BarcodeParam { barcode }): Parameters<BarcodeParam>,
    ) -> String {
        tokio::task::block_in_place(move || {
            match off::lookup_barcode(&barcode) {
                Ok(product) => {
                    serde_json::to_string_pretty(&product)
                        .unwrap_or_else(|e| format!("{{\"error\": \"Serialization failed: {e}\"}}"))
                }
                Err(e) => format!("{{\"error\": \"{e}\"}}"),
            }
        })
    }
}

// ---------------------------------------------------------------------------
// ServerHandler — server metadata
// ---------------------------------------------------------------------------

impl ServerHandler for GlowDiary {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            instructions: Some(
                "## GlowDiary — Food Diary MCP Server\n\n\
                 Track your meals, nutrition, and daily goals.\n\n\
                 **Quick start:**\n\
                 1. Call `register_user` with your name → get a UUID\n\
                 2. (Optional) Call `set_goals` to customise targets\n\
                 3. Call `search_food` to find foods via Open Food Facts\n\
                 4. Call `add_meal_from_food` with barcode + grams to log a meal\n\
                 5. Call `get_stats` or `get_weekly_stats` to see progress\n\n\
                 Every tool that needs a user_uuid expects one.\n\
                 If a user_uuid is unknown, call `register_user` first."
                    .to_string(),
            ),
            ..Default::default()
        }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        async {
            let tools = self.tool_router.list_all();
            Ok(ListToolsResult {
                tools,
                next_cursor: None,
                meta: None,
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        async {
            let ctx = ToolCallContext::new(self, request, context);
            self.tool_router.call(ctx).await
        }
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tool_router.get(name).cloned()
    }
}
