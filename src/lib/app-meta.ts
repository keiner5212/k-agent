import packageJson from "../../package.json";

export const APP_VERSION: string = packageJson.version;

export const APP_CREATORS = [
  {
    name: "Keiner Jose Alvarado Quintero",
    url: "https://github.com/keiner5212",
  },
  {
    name: "Victor Rafael Villarreal Utria",
    url: "https://github.com/Viraviutt",
  },
  {
    name: "Cristian Andres Garcia Sierra",
    url: "https://github.com/Akiii-lab",
  },
] as const;
