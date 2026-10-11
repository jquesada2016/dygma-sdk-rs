//! This module defines all possible keycodes.

use std::str::FromStr;

/// Mask used to enable/check for ctrl key modifier.
pub const CONTROL_MODIFIER: u16 = 0x0100;

/// Mask used to enable/check for alt key modifier.
pub const ALT_MODIFIER: u16 = 0x0200;

/// Mask used to enable/check for altGr key modifier.
pub const ALTGR_MODIFIER: u16 = 0x0400;

/// Mask used to enable/check for shift key modifier.
pub const SHIFT_MODIFIER: u16 = 0x0800;

/// Mask used to enable/check for OS key modifier.
pub const OS_MODIFIER: u16 = 0x1000;

/// Offset used to enable/check for ctrl dual-use functionality.
pub const CONTROL_DUAL_FUNCTION: u16 = 49169;

/// Offset used to enable/check for shift dual-use functionality.
pub const SHIFT_DUAL_FUNCTION: u16 = 49425;

/// Offset used to enable/check for alt dual-use functionality.
pub const ALT_DUAL_FUNCTION: u16 = 49681;

/// Offset used to enable/check for altGr dual-use functionality.
pub const ALTGR_DUAL_FUNCTION: u16 = 50705;

/// Offset used to enable/check for OS dual-use functionality.
pub const OS_DUAL_FUNCTION: u16 = 49937;

/// Offset used to enable/check for layer 1 dual-use functionality.
pub const LAYER_1_DUAL_FUNCTION: u16 = 51218;

/// Offset used to enable/check for layer 2 dual-use functionality.
pub const LAYER_2_DUAL_FUNCTION: u16 = 51474;

/// Offset used to enable/check for layer 3 dual-use functionality.
pub const LAYER_3_DUAL_FUNCTION: u16 = 51730;

/// Offset used to enable/check for layer 4 dual-use functionality.
pub const LAYER_4_DUAL_FUNCTION: u16 = 51986;

/// Offset used to enable/check for layer 5 dual-use functionality.
pub const LAYER_5_DUAL_FUNCTION: u16 = 52242;

/// Offset used to enable/check for layer 6 dual-use functionality.
pub const LAYER_6_DUAL_FUNCTION: u16 = 52498;

/// Offset used to enable/check for layer 7 dual-use functionality.
pub const LAYER_7_DUAL_FUNCTION: u16 = 52754;

/// Offset used to enable/check for layer 8 dual-use functionality.
pub const LAYER_8_DUAL_FUNCTION: u16 = 53010;

/// Error returned when parsing a [`str`] to a key fails.
#[derive(Clone, Copy, Debug, Display, Error)]
#[display("not a valid key")]
pub struct FromStrError;

impl serde::Serialize for KeyKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for KeyKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let s = String::deserialize(deserializer)?;

        s.parse::<Self>()
            .map_err(|_| D::Error::custom(format!("`{s}` is not a parsable key")))
    }
}

impl PartialEq<u16> for KeyKind {
    fn eq(&self, other: &u16) -> bool {
        u16::from(*self) == *other
    }
}

impl PartialEq<KeyKind> for u16 {
    fn eq(&self, other: &KeyKind) -> bool {
        u16::from(*other) == *self
    }
}

macros::generate_keycode_tables! {
  /// Blank keys.
  blank: {
    /// No Key
    NoKey = 0,
    Transparent = 65535,
  },
  /// Whitespace keys.
  #[with_modifiers]
  #[with_dual_functions]
  spacing: {
    Enter = 40,
    Escape,
    Backspace,
    Tab,
    Space,
    Insert = 73,
    Delete = 76,
  },
  /// A-Z keys.
  #[with_modifiers]
  #[with_dual_functions]
  alpha: {
    A = 4,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
  },
  /// Digits 0 - 9.
  #[with_modifiers]
  #[with_dual_functions]
  digits: {
    /// 1
    One = 30,
    /// 2
    Two,
    /// 3
    Three,
    /// 4
    Four,
    /// 5
    Five,
    /// 6
    Six,
    /// 7
    Seven,
    /// 8
    Eight,
    /// 9
    Nine,
    /// 0
    Zero,
  },
  /// Numpad keys.
  #[with_modifiers]
  #[with_dual_functions]
  numpad: {
    /// Num Lock
    NumLock = 83,
    /// Numpad /
    Divide,
    /// Numpad *
    Times,
    /// Numpad -
    Minus,
    /// Numpad +
    Add,
    /// Numpad Enter
    Enter,
    /// Numpad 1
    One,
    /// Numpad 2
    Two,
    /// Numpad 3
    Three,
    /// Numpad 4
    Four,
    /// Numpad 5
    Five,
    /// Numpad 6
    Six,
    /// Numpad 7
    Seven,
    /// Numpad 8
    Eight,
    /// Numpad 9
    Nine,
    /// Numpad 0
    Zero,
    /// Numpad .
    Period,
  },
  /// Function keys.
  #[with_modifiers]
  #[with_dual_functions]
  fx: {
    F1 = 58,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13 = 104,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
  },
  // [`Symbols`] keys with the `Shift` key applied.
  //
  // Their codes are the `Shift` modifier variants of `symbols` keys
  // (e.g. `Underscore = 2093` is `Shift + -`), so the `symbols` table
  // drops those exact expansions via `#[drop_modifier_combos(Shift)]`:
  // these named keys are the only holders of their codes, and they
  // render like Bazecor's (its DB substitutes shifted punctuation,
  // rendering 2093 as "_").
  /// Shift symbols.
  shift_symbols: {
    /// _
    Underscore = 2093,
    /// +
    Plus,
    /// {
    BraceLeft,
    /// }
    BraceRight,
    /// |
    Pipe,
    /// :
    Colon = 2099,
    /// "
    DoubleQuote,
    /// ~
    Tilde,
    /// <
    LT,
    /// >
    GT,
    /// ?
    QuestionMark,
    /// Non-U.S. |
    AltPipe = 2148,
  },
  /// Keyboard symbols, like `!`, `(`, `[`, etc.
  #[with_modifiers]
  #[with_dual_functions]
  symbols: {
    // The `Shift` expansions of the keys below are held by the named
    // `shift_symbols` keys above (e.g. `Shift + -` = `_` = 2093), so
    // generating them too would duplicate their codes.
    /// -
    #[drop_modifier_combos(Shift)]
    Dash = 45,
    /// =
    #[drop_modifier_combos(Shift)]
    Equals,
    /// [
    #[drop_modifier_combos(Shift)]
    BracketLeft,
    /// ]
    #[drop_modifier_combos(Shift)]
    BracketRight,
    /// \
    #[drop_modifier_combos(Shift)]
    Backslash,
    /// ;
    #[drop_modifier_combos(Shift)]
    Semicolon = 51,
    /// '
    #[drop_modifier_combos(Shift)]
    SingleQuote,
    /// `
    #[drop_modifier_combos(Shift)]
    BackTick,
    /// ,
    #[drop_modifier_combos(Shift)]
    Comma,
    /// .
    #[drop_modifier_combos(Shift)]
    Period,
    /// /
    #[drop_modifier_combos(Shift)]
    Slash,
    /// Caps Lock
    CapsLock,
    /// ISO <>
    // `Shift` + IsoGTLT (2148) is held by `shift_symbols::AltPipe`.
    #[drop_modifier_combos(Shift)]
    IsoGTLT = 100,
  },
  /// Modifier keys.
  #[with_modifiers]
  modifiers: {
    /// Left Ctrl
    LeftCtrl = 224,
    /// Left Shift
    LeftShift,
    /// Left Alt
    // Meh (2530) is `LeftAlt` with the `Ctrl` + `Shift` modifiers
    // (firmware `Key_Meh = LCTRL(LSHIFT(Key_LeftAlt))`); the named
    // `modifier_combos::Meh` key below holds that code.
    #[drop_modifier_combos(Ctrl + Shift)]
    LeftAlt,
    /// Left OS
    // Hyper (3043) is `LeftOs` with the `Ctrl` + `Alt` + `Shift`
    // modifiers (firmware `Key_Hyper = MEH(Key_LeftGui)`); the named
    // `modifier_combos::Hyper` key below holds that code.
    #[drop_modifier_combos(Ctrl + Alt + Shift)]
    LeftOs,
    /// Right Ctrl
    RightCtrl,
    /// Right Shift
    RightShift,
    AltGr,
    /// Right OS
    RightOs,
  },
  /// Modifier combinations.
  modifier_combos: {
    /// Meh
    Meh = 2530,
    /// Hyper
    Hyper = 3043,
  },
  /// Media keys.
  media: {
    Mute = 19682,
    /// Next Track
    NextTrack = 22709,
    /// Previous Track
    PreviousTrack,
    Stop,
    /// Play/Pause
    PlayPause = 22733,
    /// Volume Up
    VolumeUp = 23785,
    /// Volume Down
    VolumeDown,
    Eject = 22712,
    Camera = 18552,
    /// Brightness Up
    BrightnessUp = 23663,
    /// Brightness Down
    BrightnessDown,
    Calculator = 18834,
    Shuffle = 22713,
  },
  /// Navigation keys.
  #[with_modifiers]
  #[with_dual_functions]
  navigation: {
    Home = 74,
    /// Page Up
    PageUp,
    End = 77,
    /// Page Down
    PageDown,
    /// Right Arrow
    ArrowRight,
    /// Left Arrow
    ArrowLeft,
    /// Down Arrow
    ArrowDown,
    /// Up Arrow
    ArrowUp,
    Menu = 101,
  },
  /// Mouse movement.
  mouse_movement: {
    /// Mouse Up
    MouseUp = 20481,
    /// Mouse Down
    MouseDown,
    /// Mouse Left
    MouseLeft = 20484,
    /// Mouse Right
    MouseRight = 20488,

  },
  /// Mouse Wheel
  mouse_wheel: {
    /// Mouse Wheel Up
    WheelUp = 20497,
    /// Mouse Wheel Down
    WheelDown,
    /// Mouse Wheel Left
    WheelLeft = 20500,
    /// Mouse Wheel Right
    WheelRight = 20504,
  },
  /// Mouse buttons.
  mouse_buttons: {
    /// Mouse Button Left
    Left = 20545,
    /// Mouse Button Right
    Right,
    /// Mouse Button Middle
    Middle = 20548,
    /// Mouse Button Back
    Back = 20552,
    /// Mouse Button Forward
    Forward = 20560,
  },
  /// Mouse warping.
  mouse_warp: {
    /// Mouse Warp End
    End = 20576,
    /// Mouse Warp NW
    NW = 20517,
    /// Mouse Warp SW
    SW,
    /// Mouse Warp NE
    NE = 20521,
    /// Mouse Warp SE
    SE,
  },
  /// LED lighting effects.
  led_effects: {
    /// Next LED Effect
    Next = 17152,
    /// Previous LED Effect
    Previous,
    /// Toggle LED Effect
    Toggle,
  },
  /// Battery functions.
  battery: {
    /// Battery Status
    Status = 54108,
  },
  /// Bluetooth functions.
  bluetooth: {
    /// Bluetooth Pairing
    Pair = 54109,
  },
  /// Energy functions.
  energy: {
    /// Energy Status
    Status = 54111,
  },
  /// Wireless-RF functions.
  rf: {
    /// Wireless RF Status
    Status = 54112,
  },
  // The Overlay family lives only in Bazecor (`src/hw/overlay.ts`);
  // nothing in the Kaleidoscope sources models it. Bazecor's own DB
  // carries OVERLAY_KEY (54112), which collides with the RF status key
  // above — mirrored here by NOT modeling it.
  /// Overlay functions.
  overlay: {
    /// Overlay Tap
    Tap = 54113,
    /// Overlay Hold
    Hold,
  },
  /// Lock to layer keys.
  layer_lock: {
    /// Layer 1 Lock
    Layer1 = 17408,
    /// Layer 2 Lock
    Layer2,
    /// Layer 3 Lock
    Layer3,
    /// Layer 4 Lock
    Layer4,
    /// Layer 5 Lock
    Layer5,
    /// Layer 6 Lock
    Layer6,
    /// Layer 7 Lock
    Layer7,
    /// Layer 8 Lock
    Layer8,
    /// Layer 9 Lock
    Layer9,
    /// Layer 10 Lock
    Layer10,
  },
  /// Shift to layer keys.
  layer_shift: {
    /// Layer 1 Shift
    Layer1 = 17450,
    /// Layer 2 Shift
    Layer2,
    /// Layer 3 Shift
    Layer3,
    /// Layer 4 Shift
    Layer4,
    /// Layer 5 Shift
    Layer5,
    /// Layer 6 Shift
    Layer6,
    /// Layer 7 Shift
    Layer7,
    /// Layer 8 Shift
    Layer8,
    /// Layer 9 Shift
    Layer9,
    /// Layer 10 Shift
    Layer10,
  },
  /// Move to layer keys.
  layer_move: {
    /// Layer 1 Move
    Layer1 = 17492,
    /// Layer 2 Move
    Layer2,
    /// Layer 3 Move
    Layer3,
    /// Layer 4 Move
    Layer4,
    /// Layer 5 Move
    Layer5,
    /// Layer 6 Move
    Layer6,
    /// Layer 7 Move
    Layer7,
    /// Layer 8 Move
    Layer8,
    /// Layer 9 Move
    Layer9,
    /// Layer 10 Move
    Layer10,
  },
  /// miscellaneous keys.
  #[with_modifiers]
  #[with_dual_functions]
  miscellaneous: {
    /// Print Screen
    PrintScreen = 70,
    /// Scroll Lock
    ScrollLock,
    Pause,
    Shutdown = 20865,
    Sleep = 20866,
  },
  /// Oneshot keys.
  oneshot: {
    /// Oneshot Layer 1
    Layer1 = 49161,
    /// Oneshot Layer 2
    Layer2,
    /// Oneshot Layer 3
    Layer3,
    /// Oneshot Layer 4
    Layer4,
    /// Oneshot Layer 5
    Layer5,
    /// Oneshot Layer 6
    Layer6,
    /// Oneshot Layer 7
    Layer7,
    /// Oneshot Layer 8
    Layer8,
    /// Oneshot Left Ctrl
    LeftCtrl = 49153,
    /// Oneshot Left Shift
    LeftShift,
    /// Oneshot Left Alt
    LeftAlt,
    /// Oneshot Left OS
    LeftOs,
    /// Oneshot Right Ctrl
    RightCtrl,
    /// Oneshot Right Shift
    RightShift,
    /// Oneshot AltGr
    AltGr,
    /// Oneshot Right OS
    RightOs,
  },
  /// Macro keys.
  macros: {
    /// Macro 1
    Macro1 = 53852,
    /// Macro 2
    Macro2,
    /// Macro 3
    Macro3,
    /// Macro 4
    Macro4,
    /// Macro 5
    Macro5,
    /// Macro 6
    Macro6,
    /// Macro 7
    Macro7,
    /// Macro 8
    Macro8,
    /// Macro 9
    Macro9,
    /// Macro 10
    Macro10,
    /// Macro 11
    Macro11,
    /// Macro 12
    Macro12,
    /// Macro 13
    Macro13,
    /// Macro 14
    Macro14,
    /// Macro 15
    Macro15,
    /// Macro 16
    Macro16,
    /// Macro 17
    Macro17,
    /// Macro 18
    Macro18,
    /// Macro 19
    Macro19,
    /// Macro 20
    Macro20,
    /// Macro 21
    Macro21,
    /// Macro 22
    Macro22,
    /// Macro 23
    Macro23,
    /// Macro 24
    Macro24,
    /// Macro 25
    Macro25,
    /// Macro 26
    Macro26,
    /// Macro 27
    Macro27,
    /// Macro 28
    Macro28,
    /// Macro 29
    Macro29,
    /// Macro 30
    Macro30,
    /// Macro 31
    Macro31,
    /// Macro 32
    Macro32,
    /// Macro 33
    Macro33,
    /// Macro 34
    Macro34,
    /// Macro 35
    Macro35,
    /// Macro 36
    Macro36,
    /// Macro 37
    Macro37,
    /// Macro 38
    Macro38,
    /// Macro 39
    Macro39,
    /// Macro 40
    Macro40,
    /// Macro 41
    Macro41,
    /// Macro 42
    Macro42,
    /// Macro 43
    Macro43,
    /// Macro 44
    Macro44,
    /// Macro 45
    Macro45,
    /// Macro 46
    Macro46,
    /// Macro 47
    Macro47,
    /// Macro 48
    Macro48,
    /// Macro 49
    Macro49,
    /// Macro 50
    Macro50,
    /// Macro 51
    Macro51,
    /// Macro 52
    Macro52,
    /// Macro 53
    Macro53,
    /// Macro 54
    Macro54,
    /// Macro 55
    Macro55,
    /// Macro 56
    Macro56,
    /// Macro 57
    Macro57,
    /// Macro 58
    Macro58,
    /// Macro 59
    Macro59,
    /// Macro 60
    Macro60,
    /// Macro 61
    Macro61,
    /// Macro 62
    Macro62,
    /// Macro 63
    Macro63,
    /// Macro 64
    Macro64,
    /// Macro 65
    Macro65,
    /// Macro 66
    Macro66,
    /// Macro 67
    Macro67,
    /// Macro 68
    Macro68,
    /// Macro 69
    Macro69,
    /// Macro 70
    Macro70,
    /// Macro 71
    Macro71,
    /// Macro 72
    Macro72,
    /// Macro 73
    Macro73,
    /// Macro 74
    Macro74,
    /// Macro 75
    Macro75,
    /// Macro 76
    Macro76,
    /// Macro 77
    Macro77,
    /// Macro 78
    Macro78,
    /// Macro 79
    Macro79,
    /// Macro 80
    Macro80,
    /// Macro 81
    Macro81,
    /// Macro 82
    Macro82,
    /// Macro 83
    Macro83,
    /// Macro 84
    Macro84,
    /// Macro 85
    Macro85,
    /// Macro 86
    Macro86,
    /// Macro 87
    Macro87,
    /// Macro 88
    Macro88,
    /// Macro 89
    Macro89,
    /// Macro 90
    Macro90,
    /// Macro 91
    Macro91,
    /// Macro 92
    Macro92,
    /// Macro 93
    Macro93,
    /// Macro 94
    Macro94,
    /// Macro 95
    Macro95,
    /// Macro 96
    Macro96,
    /// Macro 97
    Macro97,
    /// Macro 98
    Macro98,
    /// Macro 99
    Macro99,
    /// Macro 100
    Macro100,
    /// Macro 101
    Macro101,
    /// Macro 102
    Macro102,
    /// Macro 103
    Macro103,
    /// Macro 104
    Macro104,
    /// Macro 105
    Macro105,
    /// Macro 106
    Macro106,
    /// Macro 107
    Macro107,
    /// Macro 108
    Macro108,
    /// Macro 109
    Macro109,
    /// Macro 110
    Macro110,
    /// Macro 111
    Macro111,
    /// Macro 112
    Macro112,
    /// Macro 113
    Macro113,
    /// Macro 114
    Macro114,
    /// Macro 115
    Macro115,
    /// Macro 116
    Macro116,
    /// Macro 117
    Macro117,
    /// Macro 118
    Macro118,
    /// Macro 119
    Macro119,
    /// Macro 120
    Macro120,
    /// Macro 121
    Macro121,
    /// Macro 122
    Macro122,
    /// Macro 123
    Macro123,
    /// Macro 124
    Macro124,
    /// Macro 125
    Macro125,
    /// Macro 126
    Macro126,
    /// Macro 127
    Macro127,
    /// Macro 128
    Macro128,
  },
  /// Super keys.
  super_keys: {
    /// Super Key 1
    Super1 =  53980,
    /// Super Key 2
    Super2,
    /// Super Key 3
    Super3,
    /// Super Key 4
    Super4,
    /// Super Key 5
    Super5,
    /// Super Key 6
    Super6,
    /// Super Key 7
    Super7,
    /// Super Key 8
    Super8,
    /// Super Key 9
    Super9,
    /// Super Key 10
    Super10,
    /// Super Key 11
    Super11,
    /// Super Key 12
    Super12,
    /// Super Key 13
    Super13,
    /// Super Key 14
    Super14,
    /// Super Key 15
    Super15,
    /// Super Key 16
    Super16,
    /// Super Key 17
    Super17,
    /// Super Key 18
    Super18,
    /// Super Key 19
    Super19,
    /// Super Key 20
    Super20,
    /// Super Key 21
    Super21,
    /// Super Key 22
    Super22,
    /// Super Key 23
    Super23,
    /// Super Key 24
    Super24,
    /// Super Key 25
    Super25,
    /// Super Key 26
    Super26,
    /// Super Key 27
    Super27,
    /// Super Key 28
    Super28,
    /// Super Key 29
    Super29,
    /// Super Key 30
    Super30,
    /// Super Key 31
    Super31,
    /// Super Key 32
    Super32,
    /// Super Key 33
    Super33,
    /// Super Key 34
    Super34,
    /// Super Key 35
    Super35,
    /// Super Key 36
    Super36,
    /// Super Key 37
    Super37,
    /// Super Key 38
    Super38,
    /// Super Key 39
    Super39,
    /// Super Key 40
    Super40,
    /// Super Key 41
    Super41,
    /// Super Key 42
    Super42,
    /// Super Key 43
    Super43,
    /// Super Key 44
    Super44,
    /// Super Key 45
    Super45,
    /// Super Key 46
    Super46,
    /// Super Key 47
    Super47,
    /// Super Key 48
    Super48,
    /// Super Key 49
    Super49,
    /// Super Key 50
    Super50,
    /// Super Key 51
    Super51,
    /// Super Key 52
    Super52,
    /// Super Key 53
    Super53,
    /// Super Key 54
    Super54,
    /// Super Key 55
    Super55,
    /// Super Key 56
    Super56,
    /// Super Key 57
    Super57,
    /// Super Key 58
    Super58,
    /// Super Key 59
    Super59,
    /// Super Key 60
    Super60,
    /// Super Key 61
    Super61,
    /// Super Key 62
    Super62,
    /// Super Key 63
    Super63,
    /// Super Key 64
    Super64,
    /// Super Key 65
    Super65,
    /// Super Key 66
    Super66,
    /// Super Key 67
    Super67,
    /// Super Key 68
    Super68,
    /// Super Key 69
    Super69,
    /// Super Key 70
    Super70,
    /// Super Key 71
    Super71,
    /// Super Key 72
    Super72,
    /// Super Key 73
    Super73,
    /// Super Key 74
    Super74,
    /// Super Key 75
    Super75,
    /// Super Key 76
    Super76,
    /// Super Key 77
    Super77,
    /// Super Key 78
    Super78,
    /// Super Key 79
    Super79,
    /// Super Key 80
    Super80,
    /// Super Key 81
    Super81,
    /// Super Key 82
    Super82,
    /// Super Key 83
    Super83,
    /// Super Key 84
    Super84,
    /// Super Key 85
    Super85,
    /// Super Key 86
    Super86,
    /// Super Key 87
    Super87,
    /// Super Key 88
    Super88,
    /// Super Key 89
    Super89,
    /// Super Key 90
    Super90,
    /// Super Key 91
    Super91,
    /// Super Key 92
    Super92,
    /// Super Key 93
    Super93,
    /// Super Key 94
    Super94,
    /// Super Key 95
    Super95,
    /// Super Key 96
    Super96,
    /// Super Key 97
    Super97,
    /// Super Key 98
    Super98,
    /// Super Key 99
    Super99,
    /// Super Key 100
    Super100,
    /// Super Key 101
    Super101,
    /// Super Key 102
    Super102,
    /// Super Key 103
    Super103,
    /// Super Key 104
    Super104,
    /// Super Key 105
    Super105,
    /// Super Key 106
    Super106,
    /// Super Key 107
    Super107,
    /// Super Key 108
    Super108,
    /// Super Key 109
    Super109,
    /// Super Key 110
    Super110,
    /// Super Key 111
    Super111,
    /// Super Key 112
    Super112,
    /// Super Key 113
    Super113,
    /// Super Key 114
    Super114,
    /// Super Key 115
    Super115,
    /// Super Key 116
    Super116,
    /// Super Key 117
    Super117,
    /// Super Key 118
    Super118,
    /// Super Key 119
    Super119,
    /// Super Key 120
    Super120,
    /// Super Key 121
    Super121,
    /// Super Key 122
    Super122,
    /// Super Key 123
    Super123,
    /// Super Key 124
    Super124,
    /// Super Key 125
    Super125,
    /// Super Key 126
    Super126,
    /// Super Key 127
    Super127,
    /// Super Key 128
    Super128,
      },
  /// Leader keys.
  leader: {
    /// Leader 0
    Leader0 = 53283,
    /// Leader 1
    Leader1,
    /// Leader 2
    Leader2,
    /// Leader 3
    Leader3,
    /// Leader 4
    Leader4,
    /// Leader 5
    Leader5,
    /// Leader 6
    Leader6,
    /// Leader 7
    Leader7,
  },
  // Only 16 tap dance slots exist on the firmware (53267-53282): the
  // codes right after them are the leader keys above, followed by CYCLE
  // (53291) and SYSTER (53292), which are commented out (disabled) in
  // Bazecor's DB. Bazecor's own DB carries 64 Chrysalis-inherited
  // tapdance slots that collide with the leader keys, resolved there by
  // last-write-wins toward Leader; we only model the non-colliding range.
  /// Tap dance keys.
  tap_dance: {
    /// TapDance 0
    TapDance0 = 53267,
    /// TapDance 1
    TapDance1,
    /// TapDance 2
    TapDance2,
    /// TapDance 3
    TapDance3,
    /// TapDance 4
    TapDance4,
    /// TapDance 5
    TapDance5,
    /// TapDance 6
    TapDance6,
    /// TapDance 7
    TapDance7,
    /// TapDance 8
    TapDance8,
    /// TapDance 9
    TapDance9,
    /// TapDance 10
    TapDance10,
    /// TapDance 11
    TapDance11,
    /// TapDance 12
    TapDance12,
    /// TapDance 13
    TapDance13,
    /// TapDance 14
    TapDance14,
    /// TapDance 15
    TapDance15,
  },
  // Steno and SpaceCadet follow the tapdance/leader/CYCLE/SYSTER block
  // in the firmware's `Kaleidoscope-Ranges.h` (after the unmodeled
  // TT_FIRST..TT_LAST range, 53293-53548). Steno's 42 keys are the
  // GeminiPR action enum (`geminipr::START..END`), one per code from
  // STENO_FIRST (53549) through ZR (53590); the 43rd range slot (53591,
  // `STENO_LAST`) is unused and stays unmodeled. SpaceCadet uses the
  // firmware's own `SC_FIRST`/`SC_LAST` (53592/53593), adjudicated over
  // Bazecor's DB, whose SCadet keys are one lower — see the SpaceCadet
  // table docs below.
  /// Steno keys.
  steno: {
    /// Steno FN
    Fn = 53549,
    /// Steno N1
    N1,
    /// Steno N2
    N2,
    /// Steno N3
    N3,
    /// Steno N4
    N4,
    /// Steno N5
    N5,
    /// Steno N6
    N6,
    /// Steno S1
    S1,
    /// Steno S2
    S2,
    /// Steno TL
    Tl,
    /// Steno KL
    Kl,
    /// Steno PL
    Pl,
    /// Steno WL
    Wl,
    /// Steno HL
    Hl,
    /// Steno RL
    Rl,
    /// Steno A
    A,
    /// Steno O
    O,
    /// Steno ST1
    St1,
    /// Steno ST2
    St2,
    /// Steno RE1
    Re1,
    /// Steno RE2
    Re2,
    /// Steno PWR
    Pwr,
    /// Steno ST3
    St3,
    /// Steno ST4
    St4,
    /// Steno E
    E,
    /// Steno U
    U,
    /// Steno FR
    Fr,
    /// Steno RR
    Rr,
    /// Steno PR
    Pr,
    /// Steno BR
    Br,
    /// Steno LR
    Lr,
    /// Steno GR
    Gr,
    /// Steno TR
    Tr,
    /// Steno SR
    Sr,
    /// Steno DR
    Dr,
    /// Steno N7
    N7,
    /// Steno N8
    N8,
    /// Steno N9
    N9,
    /// Steno NA
    Na,
    /// Steno NB
    Nb,
    /// Steno NC
    Nc,
    /// Steno ZR
    Zr,
  },
  /// SpaceCadet functions.
  ///
  /// Codes follow the firmware ground truth: `Kaleidoscope-Ranges.h`
  /// places `SC_FIRST`/`SC_LAST` (the `Key_SpaceCadetEnable` /
  /// `Key_SpaceCadetDisable` keys of `plugin/SpaceCadet.h`) at
  /// 53592/53593, right after the steno range whose 43rd slot (53591)
  /// is unused by GeminiPR.
  ///
  /// **Bazecor's DB spells these one lower** — its "SCadet ENABLE" is
  /// 53591 and its "SCadet DISABLE" is 53592
  /// (`src/api/keymap/db/spacecadet.ts`). Per the firmware, 53591 is a
  /// dead key and 53592 switches SpaceCadet **on**, so those spellings
  /// are known-bad for backup interop: a Bazecor backup carrying them
  /// needs remapping (relevant to backup import, WS-E).
  spacecadet: {
    /// SpaceCadet Enable
    Enable = 53592,
    /// SpaceCadet Disable
    Disable,
  },
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The generated `From<u16>` match must resolve every code without
    /// dead arms, and every named key must round-trip through its own
    /// `Display` output — the parse matchers include the display names,
    /// so a collision (two spellings for one code, or a display string
    /// that parses to a different key) would fail here.
    #[test]
    fn every_code_resolves_and_named_keys_round_trip() {
        for code in 0..=u16::MAX {
            let kind = KeyKind::from(code);

            assert_eq!(u16::from(kind), code);

            // `Unknown` has no parse path (`#[from(ignore)]`); named
            // keys must parse back from their own display name.
            if let KeyKind::Unknown(_) = kind {
                continue;
            }

            let reparsed: KeyKind = kind
                .to_string()
                .parse()
                .unwrap_or_else(|_| panic!("code {code} ({kind:?}) does not parse back"));

            assert_eq!(reparsed, kind, "code {code} does not round-trip");
        }
    }

    #[test]
    fn meh_and_hyper_are_the_named_variants() {
        assert!(matches!(
            KeyKind::from(2530),
            KeyKind::ModifierCombos(ModifierCombos::Meh)
        ));
        assert!(matches!(
            KeyKind::from(3043),
            KeyKind::ModifierCombos(ModifierCombos::Hyper)
        ));

        assert_eq!(KeyKind::from(2530).to_string(), "Meh");
        assert_eq!(KeyKind::from(3043).to_string(), "Hyper");

        assert_eq!(
            "Meh".parse::<KeyKind>().unwrap(),
            KeyKind::ModifierCombos(ModifierCombos::Meh)
        );
        assert_eq!(
            "Hyper".parse::<KeyKind>().unwrap(),
            KeyKind::ModifierCombos(ModifierCombos::Hyper)
        );
    }

    /// Ratified in issue #14: the named `shift_symbols` keys are the
    /// only holders of their codes — the generated `Shift` expansions
    /// are gone, so their spellings stop parsing (`Shift + -` becomes
    /// the dead spelling of `_`).
    #[test]
    fn shift_symbols_are_the_only_holders_of_their_codes() {
        assert!(matches!(
            KeyKind::from(2093),
            KeyKind::ShiftSymbols(ShiftSymbols::Underscore)
        ));
        assert_eq!(KeyKind::from(2093).to_string(), "_");
        assert_eq!(KeyKind::from(2148).to_string(), "Non-U.S. |");

        assert_eq!(
            "_".parse::<KeyKind>().unwrap(),
            KeyKind::ShiftSymbols(ShiftSymbols::Underscore)
        );

        for spelling in ["Shift + -", "shiftdash", "Shift + ?"] {
            assert!(
                spelling.parse::<KeyKind>().is_err(),
                "`{spelling}` should no longer parse"
            );
        }

        // `symbols::CapsLock` keeps its `Shift` expansion — it is not a
        // `shift_symbols` key.
        assert_eq!(KeyKind::from(57 + 2048).to_string(), "Shift + Caps Lock");
    }

    #[test]
    fn name_level_cleanup_from_issue_14() {
        assert_eq!(Digits::Six.to_string(), "6");
        assert_eq!(
            "6".parse::<KeyKind>().unwrap(),
            KeyKind::Digits(Digits::Six)
        );

        assert_eq!(Bluetooth::Pair.to_string(), "Bluetooth Pairing");

        assert_eq!(OS_DUAL_FUNCTION, 49937);
    }

    #[test]
    fn mouse_wheel_table_name() {
        // The table rename is compile-checked by the enum name; pin the
        // display names, which used to carry the "Wheele" typo.
        let kinds = [
            (20497, MouseWheel::WheelUp, "Mouse Wheel Up"),
            (20498, MouseWheel::WheelDown, "Mouse Wheel Down"),
            (20500, MouseWheel::WheelLeft, "Mouse Wheel Left"),
            (20504, MouseWheel::WheelRight, "Mouse Wheel Right"),
        ];

        for (code, key, display) in kinds {
            assert_eq!(KeyKind::from(code), KeyKind::MouseWheel(key));
            assert_eq!(KeyKind::from(code).to_string(), display);
        }
    }

    #[test]
    fn overlay_family() {
        assert_eq!(KeyKind::from(54113), KeyKind::Overlay(Overlay::Tap));
        assert_eq!(KeyKind::from(54114), KeyKind::Overlay(Overlay::Hold));

        assert_eq!(Overlay::Tap.to_string(), "Overlay Tap");
        assert_eq!(Overlay::Hold.to_string(), "Overlay Hold");

        // 54112 stays the RF status key: Bazecor's own DB collides
        // OVERLAY_KEY with it, and `rf::Status` mirrors that.
        assert_eq!(KeyKind::from(54112), KeyKind::Rf(Rf::Status));
    }

    #[test]
    fn spacecadet_family() {
        // Firmware ground truth (`Kaleidoscope-Ranges.h`): SC_FIRST =
        // 53592, SC_LAST = 53593. Bazecor's DB spells these one lower
        // (53591/53592) — known-bad for backup interop; see the
        // SpaceCadet table docs.
        assert_eq!(
            KeyKind::from(53592),
            KeyKind::Spacecadet(Spacecadet::Enable)
        );
        assert_eq!(
            KeyKind::from(53593),
            KeyKind::Spacecadet(Spacecadet::Disable)
        );

        assert_eq!(Spacecadet::Enable.to_string(), "SpaceCadet Enable");
        assert_eq!(Spacecadet::Disable.to_string(), "SpaceCadet Disable");
    }

    #[test]
    fn steno_family_covers_53549_through_53590() {
        let expected_displays = [
            "Steno FN",
            "Steno N1",
            "Steno N2",
            "Steno N3",
            "Steno N4",
            "Steno N5",
            "Steno N6",
            "Steno S1",
            "Steno S2",
            "Steno TL",
            "Steno KL",
            "Steno PL",
            "Steno WL",
            "Steno HL",
            "Steno RL",
            "Steno A",
            "Steno O",
            "Steno ST1",
            "Steno ST2",
            "Steno RE1",
            "Steno RE2",
            "Steno PWR",
            "Steno ST3",
            "Steno ST4",
            "Steno E",
            "Steno U",
            "Steno FR",
            "Steno RR",
            "Steno PR",
            "Steno BR",
            "Steno LR",
            "Steno GR",
            "Steno TR",
            "Steno SR",
            "Steno DR",
            "Steno N7",
            "Steno N8",
            "Steno N9",
            "Steno NA",
            "Steno NB",
            "Steno NC",
            "Steno ZR",
        ];

        for (offset, display) in expected_displays.iter().enumerate() {
            let kind = KeyKind::from(53549 + offset as u16);

            assert!(matches!(kind, KeyKind::Steno(_)));
            assert_eq!(kind.to_string(), *display);
        }

        // The firmware reserves one unused steno range slot (STENO_LAST,
        // 53591) before SpaceCadet — unmodeled, and the reason Bazecor's
        // one-lower SpaceCadet spellings are off by one.
        assert!(matches!(KeyKind::from(53591), KeyKind::Unknown(53591)));
    }
}
