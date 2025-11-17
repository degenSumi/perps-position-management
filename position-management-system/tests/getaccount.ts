import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PositionManagementSystem } from "../target/types/position_management_system";

async function checkUserAccount() {
  // Set up provider
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.PositionManagementSystem as Program<PositionManagementSystem>;
  const user = provider.wallet;

  console.log("User wallet:", user.publicKey.toString());

  // Derive user account PDA
  const [userAccountPda] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("user"), user.publicKey.toBuffer()],
    program.programId
  );

  console.log("User Account PDA:", userAccountPda.toString());

  try {
    // Fetch the account
    const userAccount = await program.account.userAccount.fetch(userAccountPda);

    console.log("\n=== User Account Details ===");
    console.log("Owner:", userAccount.owner.toString());
    console.log("Total Collateral:", userAccount.totalCollateral.toString());
    console.log("  → In USDT:", (userAccount.totalCollateral.toNumber() / 1_000_000).toFixed(2));
    console.log("Locked Collateral:", userAccount.lockedCollateral.toString());
    console.log("  → In USDT:", (userAccount.lockedCollateral.toNumber() / 1_000_000).toFixed(2));
    console.log("Available Collateral:", (userAccount.totalCollateral.sub(userAccount.lockedCollateral)).toString());
    console.log("  → In USDT:", ((userAccount.totalCollateral.sub(userAccount.lockedCollateral)).toNumber() / 1_000_000).toFixed(2));
    console.log("Total PnL:", userAccount.totalPnl.toString());
    console.log("  → In USDT:", (userAccount.totalPnl.toNumber() / 1_000_000).toFixed(2));
    console.log("Position Count:", userAccount.positionCount);
    console.log("Bump:", userAccount.bump);

  } catch (error) {
    console.error("Error fetching user account:", error);
    console.log("\nAccount might not exist yet. Initialize it first with initialize_user.");
  }
}

checkUserAccount().catch(console.error);

// const crypto = require("crypto");

// function discriminator(name) {
//   const preimage = `account:${name}`;
//   const hash = crypto.createHash("sha256").update(preimage).digest(); 
//   return hash.subarray(0, 8);
// }

// const d = discriminator("Position");

// console.log("Bytes:", [...d]);
// console.log("Hex:", d.toString("hex"));
