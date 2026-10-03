/** Present canonical Windows paths without changing the backend's stored path. */
export function formatDataFolderPath(path: string): string {
  if (/^\\\\\?\\[a-z]:\\/i.test(path)) return path.slice(4)
  if (/^\\\\\?\\UNC\\/i.test(path)) return "\\\\" + path.slice(8)
  return path
}
