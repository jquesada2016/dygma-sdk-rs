//! This module defines all the possible keycodes and their meaning.

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
pub const OSGR_DUAL_FUNCTION: u16 = 49937;

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
    /// Six
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
  /// Keyboard symbols, like `!`, `(`, `[`, etc.
  #[with_modifiers]
  #[with_dual_functions]
  symbols: {
    /// `-``
    Dash = 45,
    /// `=`
    Equals,
    /// `[`
    BracketLeft,
    /// `]`
    BracketRight,
    /// `\`
    Backslash,
    /// `;`
    Semicolon = 51,
    /// `'``
    SingleQuote,
    /// ```
    BackTick,
    /// `,`
    Comma,
    /// `.`
    Period,
    /// `/`
    Slash,
    /// Caps Lock
    CapsLock,
    /// ISO `<>`
    IsoGTLT = 100,
  },
  /// [`Symbols`] keys with the `Shift` key applied.
  shift_symbols: {
    /// `_`
    Underscore = 2093,
    /// `+`
    Plus,
    /// `{{`
    BraceLeft,
    /// `}}`
    BraceRight,
    /// `|`
    Pipe,
    /// `:`
    Colon = 2099,
    /// `"`
    DoubleQuote,
    /// `~`
    Tilde,
    /// `<`
    LT,
    /// `>`
    GT,
    /// `?`
    QuestionMark,
    /// Non-U.S. |
    AltPipe = 2148,
  },
  /// Modifier keys.
  #[with_modifiers]
  modifiers: {
    /// Left Ctrl
    LeftCtrl = 224,
    /// Left Shift
    LeftShift,
    /// Left Shift
    LeftAlt,
    /// Left OS
    LeftOs,
    /// Right Ctrl
    RightCtrl,
    /// Right Shift
    RightShift,
    AltGr,
    /// Right OS
    RightOs,
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
  mouse_wheele: {
    /// Mouse Wheele Up
    WheelUp = 20497,
    /// Mouse Wheel Down
    WheelDown,
    /// Mouse Wheel Left
    WheelLeft = 20500,
    /// Mouse Wheele Right
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
}
