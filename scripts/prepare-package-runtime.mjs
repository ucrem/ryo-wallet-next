import { prepareWalletRuntime } from "./prepare-wallet-runtime.mjs"
import { verifyLegalResources } from "./legal-resources.mjs"

await verifyLegalResources()
await prepareWalletRuntime("package")
