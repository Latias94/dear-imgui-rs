use crate::{ColormapIndex, sys};

/// Built-in colormaps
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Colormap {
    Deep = sys::ImPlotColormap_Deep as _,
    Dark = sys::ImPlotColormap_Dark as _,
    Pastel = sys::ImPlotColormap_Pastel as _,
    Paired = sys::ImPlotColormap_Paired as _,
    Viridis = sys::ImPlotColormap_Viridis as _,
    Plasma = sys::ImPlotColormap_Plasma as _,
    Hot = sys::ImPlotColormap_Hot as _,
    Cool = sys::ImPlotColormap_Cool as _,
    Pink = sys::ImPlotColormap_Pink as _,
    Jet = sys::ImPlotColormap_Jet as _,
    Twilight = sys::ImPlotColormap_Twilight as _,
    RdBu = sys::ImPlotColormap_RdBu as _,
    BrBG = sys::ImPlotColormap_BrBG as _,
    PiYG = sys::ImPlotColormap_PiYG as _,
    Spectral = sys::ImPlotColormap_Spectral as _,
    Greys = sys::ImPlotColormap_Greys as _,
}

impl Colormap {
    #[inline]
    pub const fn index(self) -> ColormapIndex {
        match ColormapIndex::from_raw(self as i32) {
            Some(index) => index,
            None => panic!("built-in ImPlot colormap index must be valid"),
        }
    }
}
