/**
 * Who made SwitchGit and how to reach them, shown in the About / Contact dialog
 * (navigation rail, or "About & contact" in the command palette). Copied from Poseidon.
 * Empty fields are hidden; links open in the default browser / mail app.
 */
import type { Author } from "@abyss/ui";

export const AUTHOR: Author = {
  name: "Hoàng Trọng Phi",
  /** Shown in the avatar; the name's first letters by default. */
  handle: "Phineas",
  role: "3D Technical Artist",
  location: "Ho Chi Minh City, Vietnam",
  /** One or two sentences: what you do, what you are open to. */
  bio: "Environments, optimization and VR/XR across Unreal, Unity and Meta Horizon, and the pipeline tools that keep the assets flowing.",
  availability: "Open to full-time roles and freelance projects",
  website: "https://huang-fei-room-porfolio.vercel.app/",
  email: "",
  skills: ["Unreal", "Unity", "Meta Horizon", "Cocos", "Web 3D", "VR/XR", "Optimization"],
  highlights: [
    { title: "Lies of P", studio: "Neowiz / Round8 Studio", years: "2021 – 2023" },
    { title: "Caliverse", studio: "Lotte", years: "2021 – 2023" },
    { title: "Net Surfer, Oblique BeatSaber", studio: "Meta Horizon Worlds", years: "2026" },
  ],
  links: [
    { label: "LinkedIn", url: "https://www.linkedin.com/in/hoangtrongphi2511/", kind: "linkedin" },
    { label: "ArtStation", url: "https://www.artstation.com/hoangphi", kind: "artstation" },
    { label: "GitHub", url: "https://github.com/HoangTrongPhi", kind: "github" },
    { label: "YouTube", url: "https://www.youtube.com/@phihoang1825/videos", kind: "youtube" },
    { label: "Facebook", url: "https://www.facebook.com/phi.hoang.568294/", kind: "facebook" },
  ],
  cv: "https://drive.google.com/file/d/1ew7p4o9LU8ICZqbRo4OMR2Ul2i61Cpie/view?usp=drive_link",
};
