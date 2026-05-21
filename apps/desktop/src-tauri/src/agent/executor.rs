use super::signal::short_id;
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: String,
    pub agent_id: String,
    pub token_symbol: String,
    pub entry_price_sol: f64,
    pub size_sol: f64,
    pub take_profit_pct: f64,
    pub stop_loss_pct: f64,
    pub opened_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeResult {
    pub position: Position,
    pub exit_price_sol: f64,
    pub pnl_sol: f64,
    pub pnl_pct: f64,
    pub closed_at: i64,
}

pub struct PaperExecutor {
    positions: Mutex<HashMap<String, Position>>,
}

impl PaperExecutor {
    pub fn new() -> Self {
        Self { positions: Mutex::new(HashMap::new()) }
    }

    pub fn open(
        &self,
        agent_id: &str,
        token: &str,
        size_sol: f64,
        entry_price_sol: f64,
        tp_pct: f64,
        sl_pct: f64,
    ) -> Result<Position> {
        if size_sol <= 0.0 {
            return Err(AppError::PaperExecutor("size must be positive".into()));
        }
        if entry_price_sol <= 0.0 {
            return Err(AppError::PaperExecutor("entry price must be positive".into()));
        }
        let pos = Position {
            id: short_id(),
            agent_id: agent_id.to_string(),
            token_symbol: token.to_string(),
            entry_price_sol,
            size_sol,
            take_profit_pct: tp_pct,
            stop_loss_pct: sl_pct,
            opened_at: chrono::Utc::now().timestamp(),
        };
        self.positions.lock().unwrap().insert(pos.id.clone(), pos.clone());
        Ok(pos)
    }

    pub fn close(&self, position_id: &str, exit_price_sol: f64) -> Result<TradeResult> {
        if exit_price_sol <= 0.0 {
            return Err(AppError::PaperExecutor("exit price must be positive".into()));
        }
        let pos = self.positions.lock().unwrap().remove(position_id)
            .ok_or_else(|| AppError::PaperExecutor(format!("position {position_id} not found")))?;
        // Token-units = size_sol / entry_price_sol. P&L_sol = units * (exit - entry).
        let units = pos.size_sol / pos.entry_price_sol;
        let pnl_sol = units * (exit_price_sol - pos.entry_price_sol);
        let pnl_pct = (exit_price_sol - pos.entry_price_sol) / pos.entry_price_sol * 100.0;
        Ok(TradeResult {
            position: pos,
            exit_price_sol,
            pnl_sol,
            pnl_pct,
            closed_at: chrono::Utc::now().timestamp(),
        })
    }

    pub fn list_for(&self, agent_id: &str) -> Vec<Position> {
        self.positions
            .lock()
            .unwrap()
            .values()
            .filter(|p| p.agent_id == agent_id)
            .cloned()
            .collect()
    }

    pub fn list_all(&self) -> Vec<Position> {
        self.positions.lock().unwrap().values().cloned().collect()
    }
}

impl Default for PaperExecutor {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_creates_position() {
        let ex = PaperExecutor::new();
        let pos = ex.open("agent-1", "BONK", 0.5, 0.0001, 50.0, 20.0).unwrap();
        assert_eq!(pos.agent_id, "agent-1");
        assert_eq!(pos.size_sol, 0.5);
        assert_eq!(ex.list_for("agent-1").len(), 1);
    }

    #[test]
    fn open_rejects_zero_size() {
        let ex = PaperExecutor::new();
        assert!(ex.open("a", "BONK", 0.0, 0.0001, 50.0, 20.0).is_err());
    }

    #[test]
    fn close_at_double_price_doubles_position_value() {
        let ex = PaperExecutor::new();
        let pos = ex.open("a", "BONK", 0.5, 0.0001, 50.0, 20.0).unwrap();
        let res = ex.close(&pos.id, 0.0002).unwrap();
        // Bought 5000 units at 0.0001, sold at 0.0002 → +0.5 SOL
        assert!((res.pnl_sol - 0.5).abs() < 1e-9);
        assert!((res.pnl_pct - 100.0).abs() < 1e-9);
    }

    #[test]
    fn close_at_half_price_loses_half() {
        let ex = PaperExecutor::new();
        let pos = ex.open("a", "WIF", 1.0, 1.0, 100.0, 50.0).unwrap();
        let res = ex.close(&pos.id, 0.5).unwrap();
        assert!((res.pnl_sol - (-0.5)).abs() < 1e-9);
        assert!((res.pnl_pct - (-50.0)).abs() < 1e-9);
    }

    #[test]
    fn close_removes_from_open_list() {
        let ex = PaperExecutor::new();
        let pos = ex.open("a", "BONK", 0.5, 0.0001, 50.0, 20.0).unwrap();
        ex.close(&pos.id, 0.0001).unwrap();
        assert!(ex.list_for("a").is_empty());
    }

    #[test]
    fn close_unknown_position_errors() {
        let ex = PaperExecutor::new();
        assert!(ex.close("nope", 1.0).is_err());
    }

    #[test]
    fn list_for_isolates_by_agent() {
        let ex = PaperExecutor::new();
        ex.open("a", "X", 0.5, 1.0, 10.0, 10.0).unwrap();
        ex.open("b", "Y", 0.5, 1.0, 10.0, 10.0).unwrap();
        assert_eq!(ex.list_for("a").len(), 1);
        assert_eq!(ex.list_for("b").len(), 1);
        assert_eq!(ex.list_all().len(), 2);
    }
}
