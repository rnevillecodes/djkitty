# Spec Delta

## Purpose

Keeps a DJ's whole library (audio, analysis, hot cues, grids and sets) in one folder the user chooses: a vault. A vault is safe from changes to the original files, and it can be moved or opened on another computer as it is.

## ADDED Requirements

### Requirement: A vault is required
The system SHALL keep the library only inside a vault and SHALL NOT show the library, import, or let tracks be loaded until a vault is open. When no vault has been opened on this computer, the system SHALL show a welcome screen offering to create a new vault or open an existing one.

#### Scenario: First launch
- **WHEN** djkitty starts on a computer where no vault has ever been opened
- **THEN** the welcome screen offers "Create new vault" and "Open existing vault", and the library is not shown

### Requirement: Create a vault
The system SHALL create a vault from a name the user enters and a parent folder the user chooses, as a new folder `<parent>\<name>` containing `.djkitty`, `Catalog` and `Sets` folders. The system SHALL refuse an empty name, a name Windows does not allow as a folder name, or a target folder that already exists, and say why. The new vault SHALL open immediately.

#### Scenario: Create a vault on drive D
- **WHEN** the user enters the name "My DJ Vault", chooses `D:\` and confirms
- **THEN** `D:\My DJ Vault` is created with `.djkitty`, `Catalog` and `Sets` inside it, and the empty library opens

#### Scenario: Folder already exists
- **WHEN** the user enters a name and parent folder where a folder of that name already exists
- **THEN** no vault is created and the user is told that folder already exists

#### Scenario: Invalid name
- **WHEN** the user enters a name containing `:` or `?`, or a name made only of spaces
- **THEN** the vault cannot be created and the user is told the name is not allowed

### Requirement: Open an existing vault
The system SHALL let the user open a vault by choosing its folder. A folder SHALL count as a vault only if it contains djkitty's vault marker, and the system SHALL refuse any other folder without creating anything in it.

#### Scenario: Open a vault from a USB stick
- **WHEN** the user chooses "Open existing vault" and picks `E:\My DJ Vault`, a vault created on another computer
- **THEN** the vault opens with all its tracks, analysis, hot cues, beatgrids and sets

#### Scenario: Not a vault
- **WHEN** the user picks a folder that is not a vault, such as `D:\Music`
- **THEN** the user is told the folder is not a djkitty vault, and nothing is created or changed in it

### Requirement: Vaults are portable
The system SHALL store every reference from the library to a vault file relative to the vault folder, so that a vault keeps working, with no broken links, after it is moved, renamed, opened from a different drive letter or opened on another computer.

#### Scenario: Vault moved to another drive
- **WHEN** the user copies the whole vault folder from `D:\` to `E:\Gig Vault` and opens it there
- **THEN** every track loads, and no track shows a broken link

### Requirement: Last vault reopens on launch
The system SHALL remember on each computer which vault was last open and SHALL open it automatically on the next launch. The user SHALL be able to switch to another vault, or create one, without restarting the app, while no deck is playing.

#### Scenario: Restart
- **WHEN** the user has vault "My DJ Vault" open, closes djkitty and starts it again
- **THEN** "My DJ Vault" opens without showing the welcome screen

#### Scenario: Switch vault
- **WHEN** the user chooses to open a different vault while one is open
- **THEN** the decks are emptied, the other vault opens, and it is remembered as the last vault

#### Scenario: Switch while a deck is playing
- **WHEN** the user chooses to open a different vault while deck A is playing
- **THEN** the vault does not change and the user is told to stop both decks first

### Requirement: Unavailable vault
When the last vault cannot be opened at launch, for example because its drive is not connected, the system SHALL show the welcome screen with a message naming the vault and saying it is not available. The system SHALL NOT create a new vault or an empty library in its place.

#### Scenario: USB stick not plugged in
- **WHEN** the last vault was `E:\Gig Vault` and djkitty starts with drive E not connected
- **THEN** the welcome screen says `E:\Gig Vault` is not available and offers to open or create a vault, and nothing is created on any drive

### Requirement: Warning for cloud-synced folders
The system SHALL warn the user when they create or open a vault inside a folder synced by OneDrive, Dropbox or iCloud Drive, explaining that syncing can damage the vault's library database, and SHALL let them continue or choose another folder.

#### Scenario: Vault in OneDrive
- **WHEN** the user creates a vault inside their OneDrive folder
- **THEN** a warning explains the risk and offers "Continue anyway" or "Choose another folder"

### Requirement: Recovery from an interrupted import
The system SHALL leave the vault consistent if djkitty stops during an import: when the vault is next opened, every track in the library SHALL have a complete Catalog file, and partly copied files SHALL be removed.

#### Scenario: Power cut during import
- **WHEN** the computer loses power while a large import is copying files and the vault is opened again
- **THEN** the tracks copied before the cut are in the library and load normally, and no partial file remains in the vault
