//! Headless lobshot: the same sim and referee the server runs, with no clocks or sockets. Balance
//! runs play thousands of SearchBot games and report how the mobiles fare (`balance`, `report`);
//! LLM agents play turn by turn through a JSON interface (M4). See `DESIGN.md`, "Bots and the lab".

pub mod agent;
pub mod balance;
pub mod host;
pub mod mcp;
pub mod report;
