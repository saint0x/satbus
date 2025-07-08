use crate::protocol::Command;
use heapless::Vec;
use serde::{Deserialize, Serialize};

const MAX_SCHEDULED_COMMANDS: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledCommand {
    pub command: Command,
    pub execution_time: u64,
    pub scheduled_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SchedulerStats {
    pub total_scheduled: u32,
    pub total_executed: u32,
    pub total_expired: u32,
    pub currently_scheduled: u8,
}

#[derive(Debug)]
pub struct CommandScheduler {
    scheduled_commands: Vec<ScheduledCommand, MAX_SCHEDULED_COMMANDS>,
    stats: SchedulerStats,
    command_timeout_seconds: u64,
}

impl CommandScheduler {
    pub fn new() -> Self {
        Self {
            scheduled_commands: Vec::new(),
            stats: SchedulerStats::default(),
            command_timeout_seconds: 3600, // 1 hour timeout by default
        }
    }
    
    /// Schedule a command for future execution
    pub fn schedule_command(&mut self, command: Command, current_time: u64) -> Result<(), &'static str> {
        // NASA Rule 5: Safety assertion for scheduler capacity
        debug_assert!(
            self.scheduled_commands.len() < MAX_SCHEDULED_COMMANDS,
            "Scheduler queue length {} at capacity {}", 
            self.scheduled_commands.len(), MAX_SCHEDULED_COMMANDS
        );
        
        let execution_time = command.execution_time.unwrap_or(current_time);
        
        // Validate execution time is not too far in the future
        if execution_time > current_time + (self.command_timeout_seconds * 1000) {
            return Err("Execution time too far in future");
        }
        
        // Validate execution time is not in the past (with small tolerance for clock skew)
        if execution_time < current_time.saturating_sub(5000) { // 5 second tolerance
            return Err("Execution time in the past");
        }
        
        let scheduled_command = ScheduledCommand {
            command,
            execution_time,
            scheduled_at: current_time,
        };
        
        // Insert in chronological order
        let insert_position = self.scheduled_commands
            .iter()
            .position(|cmd| cmd.execution_time > execution_time)
            .unwrap_or(self.scheduled_commands.len());
        
        if self.scheduled_commands.is_full() {
            return Err("Scheduler queue full");
        }
        
        // Shift elements to make room
        if insert_position < self.scheduled_commands.len() {
            // We need to insert at a specific position, but heapless::Vec doesn't have insert
            // So we'll add to the end and then sort
            let _ = self.scheduled_commands.push(scheduled_command);
            
            // Sort by execution time to maintain order
            self.scheduled_commands.sort_by_key(|cmd| cmd.execution_time);
        } else {
            let _ = self.scheduled_commands.push(scheduled_command);
        }
        
        self.stats.total_scheduled += 1;
        self.stats.currently_scheduled = self.scheduled_commands.len() as u8;
        
        Ok(())
    }
    
    /// Get commands ready for execution
    pub fn get_ready_commands(&mut self, current_time: u64) -> Vec<Command, 8> {
        let mut ready_commands: Vec<Command, 8> = Vec::new();
        let mut commands_to_remove = Vec::<usize, 8>::new();
        
        // Find commands ready for execution
        for (index, scheduled_cmd) in self.scheduled_commands.iter().enumerate() {
            if scheduled_cmd.execution_time <= current_time {
                if ready_commands.push(scheduled_cmd.command.clone()).is_ok() {
                    let _ = commands_to_remove.push(index);
                } else {
                    // Ready commands buffer full, will process remaining next cycle
                    break;
                }
            } else {
                // Commands are sorted by execution time, so we can stop here
                break;
            }
        }
        
        // Remove executed commands in reverse order to maintain indices
        // Use regular remove() instead of swap_remove() to preserve chronological order
        for &index in commands_to_remove.iter().rev() {
            self.scheduled_commands.remove(index);
            self.stats.total_executed += 1;
        }
        
        self.stats.currently_scheduled = self.scheduled_commands.len() as u8;
        
        ready_commands
    }
    
    /// Clean up expired commands
    pub fn cleanup_expired_commands(&mut self, current_time: u64) {
        let timeout_threshold = current_time.saturating_sub(self.command_timeout_seconds * 1000);
        let initial_count = self.scheduled_commands.len();
        
        self.scheduled_commands.retain(|cmd| {
            cmd.scheduled_at > timeout_threshold
        });
        
        let expired_count = initial_count - self.scheduled_commands.len();
        self.stats.total_expired += expired_count as u32;
        self.stats.currently_scheduled = self.scheduled_commands.len() as u8;
    }
    
    /// Get scheduler statistics
    pub fn get_stats(&self) -> &SchedulerStats {
        &self.stats
    }
    
    /// Get currently scheduled commands
    pub fn get_scheduled_commands(&self) -> &[ScheduledCommand] {
        &self.scheduled_commands
    }
    
    /// Clear all scheduled commands
    pub fn clear_all_scheduled(&mut self) {
        let cleared_count = self.scheduled_commands.len();
        self.scheduled_commands.clear();
        self.stats.total_expired += cleared_count as u32;
        self.stats.currently_scheduled = 0;
    }
    
    /// Set command timeout
    pub fn set_timeout_seconds(&mut self, timeout_seconds: u64) {
        self.command_timeout_seconds = timeout_seconds;
    }
}

impl Default for CommandScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::CommandType;
    
    fn create_test_command(id: u32, execution_time: Option<u64>) -> Command {
        Command {
            id,
            timestamp: 1000,
            command_type: CommandType::Ping,
            execution_time,
        }
    }
    
    #[test]
    fn test_scheduler_creation() {
        let scheduler = CommandScheduler::new();
        assert_eq!(scheduler.scheduled_commands.len(), 0);
        assert_eq!(scheduler.stats.total_scheduled, 0);
    }
    
    #[test]
    fn test_immediate_command_scheduling() {
        let mut scheduler = CommandScheduler::new();
        let current_time = 1000;
        
        let command = create_test_command(1, Some(current_time));
        let result = scheduler.schedule_command(command, current_time);
        assert!(result.is_ok());
        
        let ready = scheduler.get_ready_commands(current_time);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, 1);
    }
    
    #[test]
    fn test_future_command_scheduling() {
        let mut scheduler = CommandScheduler::new();
        let current_time = 1000;
        let future_time = current_time + 5000;
        
        let command = create_test_command(1, Some(future_time));
        let result = scheduler.schedule_command(command, current_time);
        assert!(result.is_ok());
        
        // Should not be ready yet
        let ready = scheduler.get_ready_commands(current_time);
        assert_eq!(ready.len(), 0);
        
        // Should be ready at future time
        let ready = scheduler.get_ready_commands(future_time);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, 1);
    }
    
    #[test]
    fn test_command_ordering() {
        let mut scheduler = CommandScheduler::new();
        let current_time = 1000;
        
        // Schedule commands out of order
        let cmd3 = create_test_command(3, Some(current_time + 3000));
        let cmd1 = create_test_command(1, Some(current_time + 1000));
        let cmd2 = create_test_command(2, Some(current_time + 2000));
        
        scheduler.schedule_command(cmd3, current_time).unwrap();
        scheduler.schedule_command(cmd1, current_time).unwrap();
        scheduler.schedule_command(cmd2, current_time).unwrap();
        
        
        // Commands should be executed in chronological order
        // At time 1000, only command 1 should be ready
        let ready1 = scheduler.get_ready_commands(current_time + 1000);
        assert_eq!(ready1.len(), 1);
        assert_eq!(ready1[0].id, 1);
        
        // At time 2000, only command 2 should be ready (command 1 already executed)
        let ready2 = scheduler.get_ready_commands(current_time + 2000);
        assert_eq!(ready2.len(), 1);
        assert_eq!(ready2[0].id, 2);
        
        // At time 3000, only command 3 should be ready (commands 1&2 already executed)
        let ready3 = scheduler.get_ready_commands(current_time + 3000);
        assert_eq!(ready3.len(), 1);
        assert_eq!(ready3[0].id, 3);
    }
    
    #[test]
    fn test_past_command_rejection() {
        let mut scheduler = CommandScheduler::new();
        let current_time = 10000;
        let past_time = current_time - 10000; // 10 seconds ago
        
        let command = create_test_command(1, Some(past_time));
        let result = scheduler.schedule_command(command, current_time);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_command_cleanup() {
        let mut scheduler = CommandScheduler::new();
        scheduler.set_timeout_seconds(5); // 5 second timeout
        
        let current_time = 1000;
        let command = create_test_command(1, Some(current_time + 1000));
        scheduler.schedule_command(command, current_time).unwrap();
        
        // Fast forward past timeout
        let future_time = current_time + 10000;
        scheduler.cleanup_expired_commands(future_time);
        
        assert_eq!(scheduler.scheduled_commands.len(), 0);
        assert_eq!(scheduler.stats.total_expired, 1);
    }
}