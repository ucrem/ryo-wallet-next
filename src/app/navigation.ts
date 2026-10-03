export type Screen = "home" | "storage" | "node" | "summary" | "wallet" | "settings" | "about"
export type NavigationLayout = "sidebar" | "top"

export const navigation: { screen: Screen; label: string; number: string }[] = [
  { screen: "home", label: "Start", number: "◆" },
  { screen: "storage", label: "Data location", number: "01" },
  { screen: "node", label: "Node", number: "02" },
  { screen: "summary", label: "Summary", number: "03" },
  { screen: "wallet", label: "Wallet", number: "04" },
]
