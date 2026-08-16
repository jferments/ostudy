export const localFontSources = Object.freeze([
  {
    id: "plex",
    family: "IBM Plex Sans",
    googleWeights: "300..700",
    faces: [
      {
        googleWeight: "300 700",
        cssWeight: "300 700",
        filename: "ibm-plex-sans-latin-variable.woff2",
      },
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
]);
