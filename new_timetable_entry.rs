#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct TimetableEntry2 {
    /// Arrangement
    /// ```
    /// P.AADD.. NNNNNNNN NNNNNNNN NNNNNNNN
    ///
    /// P: Pinned state. Pinned if set; derived if unset
    /// A: Arrval state:
    ///   00: Flexible
    ///   01: At. The arr field would be interpreted as timepoint
    ///   10: For. The arr field would be interpreted as duration
    ///   11: Non-stop
    /// D: Departure state:
    ///   00: Flexible
    ///   01: At. The dep field would be interpreted as timepoint
    ///   10: For. The arr field would be interpreted as duration
    ///   11: (undefined)
    /// ```
    fd1: u32,
    fd2: u32,
    stn: Option<Entity>,
}

const _: () = assert!(
    size_of::<TimetableEntry2>() == 16,
    "The size of TimetableEntry2 should be exactly 16 bytes."
);

impl TimetableEntry2 {
    pub fn new(view: TimetableEntryView) -> Self {
        view.into()
    }
}

#[inline]
fn sign_extend_24(x: u32) -> i32 {
    ((x as i32) << 8) >> 8
}

impl From<TimetableEntry2> for TimetableEntryView {
    fn from(value: TimetableEntry2) -> Self {
        if value.fd1 >> 31 != 0 {
            return TimetableEntryView::Derived(value.stn.unwrap());
        }
        let Some(stn) = value.stn else {
            return TimetableEntryView::Pinned(None);
        };
        let arr_val = sign_extend_24(value.fd1 & 0x00FF_FFFF);
        let dep_val = sign_extend_24(value.fd2 & 0x00FF_FFFF);

        // arrival
        let arr = match value.fd1 >> 28 & 0b11 {
            0b00 => Some(TravelMode::Flexible),
            0b01 => Some(TravelMode::At(TimetableTime(arr_val))),
            0b10 => Some(TravelMode::For(Duration(arr_val))),
            0b11 => None,
            _ => unreachable!(),
        };

        // departure
        let dep = match value.fd1 >> 26 & 0b11 {
            // Treat 0b00 and 0b11 as the same in the implementation
            0b00 | 0b11 => TravelMode::Flexible,
            0b01 => TravelMode::At(TimetableTime(dep_val)),
            0b10 => TravelMode::For(Duration(dep_val)),
            _ => unreachable!(),
        };
        let view = PinnedView { arr, dep, stn };
        TimetableEntryView::Pinned(Some(view))
    }
}

impl From<TimetableEntryView> for TimetableEntry2 {
    fn from(value: TimetableEntryView) -> Self {
        let inner: Option<PinnedView>;
        match value {
            TimetableEntryView::Pinned(d) => inner = d,
            TimetableEntryView::Derived(e) => {
                return Self {
                    fd1: 0,
                    fd2: 0,
                    stn: Some(e),
                };
            }
        };
        let Some(PinnedView { arr, dep, stn }) = inner else {
            return Self {
                fd1: 1 << 31,
                fd2: 0,
                stn: None,
            };
        };
        Self {
            fd1: 0,
            fd2: 0,
            stn: Some(stn),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TimetableEntryView {
    Derived(Entity),
    Pinned(Option<PinnedView>),
}

#[derive(Clone, Copy, Debug)]
pub struct PinnedView {
    arr: Option<TravelMode>,
    dep: TravelMode,
    stn: Entity,
}
