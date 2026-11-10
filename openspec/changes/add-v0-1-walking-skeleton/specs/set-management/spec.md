# Spec Delta

## Purpose

Lets DJs plan what they will play by building named, ordered sets from library tracks, and load tracks from a set onto the decks when performing.

## ADDED Requirements

### Requirement: Create, rename and delete sets
The system SHALL let the user create a set with a name, rename it and delete it. Deleting a set SHALL NOT remove its tracks from the library.

#### Scenario: Create a set
- **WHEN** the user creates a set named "Friday warmup"
- **THEN** an empty set with that name appears in the sets list

#### Scenario: Delete a set
- **WHEN** the user deletes a set that contains 5 tracks
- **THEN** the set is gone and all 5 tracks remain in the library

### Requirement: Add and remove tracks
The system SHALL let the user add library tracks to a set and remove them from it. A track SHALL be allowed in more than one set, but adding a track a set already contains SHALL NOT add a duplicate.

#### Scenario: Add a track
- **WHEN** the user adds a library track to a set
- **THEN** the track appears at the end of that set

#### Scenario: Add a duplicate
- **WHEN** the user adds a track that is already in the set
- **THEN** the set is unchanged

### Requirement: Reorder tracks
The system SHALL let the user change the order of tracks within a set, and the order SHALL be persisted.

#### Scenario: Move a track up
- **WHEN** the user moves the 4th track of a set to position 2
- **THEN** the set lists that track second and the order is the same after restarting the app

### Requirement: Set overview
The system SHALL show each track's position, title, artist, BPM, key and duration within a set, plus the set's track count and total duration.

#### Scenario: Total duration
- **WHEN** a set contains tracks of 5:00, 6:30 and 4:15
- **THEN** the set shows 3 tracks and a total of 15:45

### Requirement: Load from a set
The system SHALL let the user load any track in a set onto either deck.

#### Scenario: Load to deck B
- **WHEN** the user chooses "load to deck B" on the 2nd track of a set
- **THEN** that track is loaded onto deck B
