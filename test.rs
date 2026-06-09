struct Trip {
    // This should actually be stored in a secondary map
    veh: Vec<usize>,
    // every vehicle must have a schedule, so this is mandatory
    sch: Vec<TimetableEntry>,
    // reference to the class, which stores some sort of colour and stroke info
    // The class is totally optional. A value of None represents no class
    cls: Option<usize>,
}

struct TripStorage {}

struct Vehicle {}

struct TimetableEntry;

struct AppStorage {
    // TODO
}

// and then we define a command system that covers all the edge cases
