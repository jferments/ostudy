export const localFontSources = Object.freeze([
  {
    id: "plex",
    family: "IBM Plex Sans",
    googleWeights: "300..700",
    license: {
      spdx: "OFL-1.1",
      noticePath: "public/licenses/IBM-Plex-Sans-OFL-1.1.txt",
      copyright:
        "Copyright 2017 The IBM Plex Project Authors (https://github.com/IBM/plex)",
    },
    faces: [
      {
        googleWeight: "300 700",
        cssWeight: "300 700",
        filename: "ibm-plex-sans-latin-variable.woff2",
      },
    ],
  },
  {
    id: "plex-mono",
    family: "IBM Plex Mono",
    // Console's heading and wordmark face (theme-owned, not a selectable preset).
    // Google Fonts ships it as static faces; headings use 600 and the wordmark 700.
    selectable: false,
    googleWeights: "600;700",
    license: {
      spdx: "OFL-1.1",
      // One OFL notice covers the whole IBM Plex family.
      noticePath: "public/licenses/IBM-Plex-Sans-OFL-1.1.txt",
      copyright:
        "Copyright 2017 The IBM Plex Project Authors (https://github.com/IBM/plex)",
    },
    faces: [
      { googleWeight: "600", cssWeight: "600", filename: "ibm-plex-mono-latin-600.woff2" },
      { googleWeight: "700", cssWeight: "700", filename: "ibm-plex-mono-latin-700.woff2" },
    ],
  },
  {
    id: "albert",
    family: "Albert Sans",
    googleWeights: "300..700",
    faces: [
      {
        googleWeight: "300 700",
        cssWeight: "300 700",
        filename: "albert-sans-latin-variable.woff2",
      },
    ],
  },
  {
    id: "space-grotesk",
    family: "Space Grotesk",
    googleWeights: "300..700",
    faces: [
      {
        googleWeight: "300 700",
        cssWeight: "300 700",
        filename: "space-grotesk-latin-variable.woff2",
      },
    ],
  },
  {
    id: "spline-sans",
    family: "Spline Sans",
    googleWeights: "300..700",
    faces: [
      {
        googleWeight: "300 700",
        cssWeight: "300 700",
        filename: "spline-sans-latin-variable.woff2",
      },
    ],
  },
  {
    id: "sora",
    family: "Sora",
    googleWeights: "300..700",
    faces: [
      {
        googleWeight: "300 700",
        cssWeight: "300 700",
        filename: "sora-latin-variable.woff2",
      },
    ],
  },
  {
    id: "roboto",
    family: "Roboto",
    googleWeights: "300..700",
    faces: [
      {
        googleWeight: "300 700",
        cssWeight: "300 700",
        filename: "roboto-latin-variable.woff2",
      },
    ],
  },
  {
    id: "roboto-slab",
    family: "Roboto Slab",
    googleWeights: "300..700",
    faces: [
      {
        googleWeight: "300 700",
        cssWeight: "300 700",
        filename: "roboto-slab-latin-variable.woff2",
      },
    ],
  },
  {
    id: "sofia-sans",
    family: "Sofia Sans",
    googleWeights: "300..700",
    faces: [
      {
        googleWeight: "300 700",
        cssWeight: "300 700",
        filename: "sofia-sans-latin-variable.woff2",
      },
    ],
  },
  {
    id: "lato",
    family: "Lato",
    // Google Fonts distributes Lato as static faces rather than a variable
    // weight font, so keep the three weights ostudy's hierarchy actually uses.
    googleWeights: "300;400;700",
    faces: [
      { googleWeight: "300", cssWeight: "300", filename: "lato-latin-300.woff2" },
      { googleWeight: "400", cssWeight: "400", filename: "lato-latin-400.woff2" },
      { googleWeight: "700", cssWeight: "700", filename: "lato-latin-700.woff2" },
    ],
  },
  {
    id: "ubuntu",
    family: "Ubuntu",
    // The Google Fonts web build exposes Ubuntu as static faces. Include the
    // medium face because many controls use a 500-weight emphasis.
    googleWeights: "300;400;500;700",
    faces: [
      { googleWeight: "300", cssWeight: "300", filename: "ubuntu-latin-300.woff2" },
      { googleWeight: "400", cssWeight: "400", filename: "ubuntu-latin-400.woff2" },
      { googleWeight: "500", cssWeight: "500", filename: "ubuntu-latin-500.woff2" },
      { googleWeight: "700", cssWeight: "700", filename: "ubuntu-latin-700.woff2" },
    ],
  },
]);

// TrueType faces embedded in exported study-guide PDFs. Typst cannot read
// WOFF2, so these come from Google Fonts' TrueType stylesheet and are compiled
// into the native binary from src-tauri/resources/pdf-fonts. The serif body
// face (Libertinus Serif) and the math face come from the typst-assets crate.
export const pdfFontDirectory = "src-tauri/resources/pdf-fonts";

export const pdfFontSources = Object.freeze([
  {
    family: "IBM Plex Sans",
    googleAxes: "ital,wght@0,400;0,600;0,700;1,400",
    noticePath: "public/licenses/IBM-Plex-Sans-OFL-1.1.txt",
    faces: [
      { style: "normal", weight: "400", filename: "ibm-plex-sans-400.ttf" },
      { style: "italic", weight: "400", filename: "ibm-plex-sans-400-italic.ttf" },
      { style: "normal", weight: "600", filename: "ibm-plex-sans-600.ttf" },
      { style: "normal", weight: "700", filename: "ibm-plex-sans-700.ttf" },
    ],
  },
  {
    family: "Lato",
    googleAxes: "ital,wght@0,400;0,700;1,400",
    noticePath: "public/licenses/Lato-OFL-1.1.txt",
    faces: [
      { style: "normal", weight: "400", filename: "lato-400.ttf" },
      { style: "italic", weight: "400", filename: "lato-400-italic.ttf" },
      { style: "normal", weight: "700", filename: "lato-700.ttf" },
    ],
  },
  {
    family: "Sora",
    googleAxes: "wght@400;600;700",
    noticePath: "public/licenses/Sora-OFL-1.1.txt",
    faces: [
      { style: "normal", weight: "400", filename: "sora-400.ttf" },
      { style: "normal", weight: "600", filename: "sora-600.ttf" },
      { style: "normal", weight: "700", filename: "sora-700.ttf" },
    ],
  },
  {
    family: "Roboto Slab",
    googleAxes: "wght@400;700",
    noticePath: "public/licenses/Roboto-Slab-Apache-2.0.txt",
    faces: [
      { style: "normal", weight: "400", filename: "roboto-slab-400.ttf" },
      { style: "normal", weight: "700", filename: "roboto-slab-700.ttf" },
    ],
  },
  {
    family: "Caladea",
    googleAxes: "wght@400;700",
    noticePath: "public/licenses/Caladea-OFL-1.1.txt",
    faces: [
      { style: "normal", weight: "400", filename: "caladea-400.ttf" },
      { style: "normal", weight: "700", filename: "caladea-700.ttf" },
    ],
  },
]);
