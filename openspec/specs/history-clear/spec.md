## Purpose

Allow users to delete all clipboard synchronization history records with a single action, with proper confirmation to prevent accidental data loss.

## Requirements

### Requirement: Clear all history
The system SHALL allow users to delete all clipboard history records with a single action.

#### Scenario: Clear with confirmation
- **WHEN** user clicks "清空全部历史" button in the history tab
- **THEN** a confirmation dialog SHALL appear asking "确定要清空所有历史记录？此操作不可撤销。"
- **AND** the dialog SHALL provide "确定" and "取消" buttons

#### Scenario: Cancel clears nothing
- **WHEN** user clicks "取消" in the confirmation dialog
- **THEN** no history records SHALL be deleted

#### Scenario: Confirm deletes all records
- **WHEN** user clicks "确定" in the confirmation dialog
- **THEN** all records in clipboard_history table SHALL be deleted
- **AND** the history tab SHALL display the empty state

#### Scenario: Clear after new records
- **WHEN** user clears all history
- **AND** new clipboard activity generates new records
- **THEN** new records SHALL appear in the history tab normally
