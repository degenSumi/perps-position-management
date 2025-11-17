import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PositionManagementSystem } from "../target/types/position_management_system";
import { expect } from "chai";

describe("position-management-system", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace
    .PositionManagementSystem as Program<PositionManagementSystem>;
  const user = provider.wallet;

  it("Initialize user account", async () => {
    // Derive PDA for verification (not for passing to .accounts())
    const [userAccountPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("user"), user.publicKey.toBuffer()],
      program.programId
    );

    try {
      // Only pass signer accounts - PDAs are auto-resolved!
      const tx = await program.methods.initializeUser().rpc();

      console.log("Initialize user tx:", tx);

      // Fetch the account to verify
      const userAccount = await program.account.userAccount.fetch(
        userAccountPda
      );
      console.log("User account:", userAccount);
      expect(userAccount.positionCount).to.equal(0);
      expect(userAccount.totalCollateral.toString()).to.equal("0");
    } catch (error) {
      console.error("Error:", error);
      throw error;
    }
  });

  it("Check user account details", async () => {
    const [userAccountPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("user"), user.publicKey.toBuffer()],
      program.programId
    );

    const userAccount = await program.account.userAccount.fetch(userAccountPda);

    console.log("PDA:", userAccountPda.toString());

    console.log("\n=== User Account Details ===");
    console.log(
      "Total Collateral (raw):",
      userAccount.totalCollateral.toString()
    );
    console.log(
      "Total Collateral (USDT):",
      (userAccount.totalCollateral.toNumber() / 1_000_000).toFixed(2)
    );
    console.log(
      "Locked Collateral (raw):",
      userAccount.lockedCollateral.toString()
    );
    console.log(
      "Locked Collateral (USDT):",
      (userAccount.lockedCollateral.toNumber() / 1_000_000).toFixed(2)
    );

    const available = userAccount.totalCollateral.sub(
      userAccount.lockedCollateral
    );
    console.log("Available Collateral (raw):", available.toString());
    console.log(
      "Available Collateral (USDT):",
      (available.toNumber() / 1_000_000).toFixed(2)
    );

    console.log("Total PnL (raw):", userAccount.totalPnl.toString());
    console.log("Position Count:", userAccount.positionCount);
  });

  it("Display all user positions", async () => {
    const [userAccountPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("user"), user.publicKey.toBuffer()],
      program.programId
    );

    try {
      // Fetch user account to get position count
      const userAccount = await program.account.userAccount.fetch(
        userAccountPda
      );

      console.log("\n=== User Account ===");
      console.log("Owner:", userAccount.owner.toString());
      console.log("Position Count:", userAccount.positionCount);
      console.log(
        "Total Collateral:",
        userAccount.totalCollateral.toString(),
        `(${(userAccount.totalCollateral.toNumber() / 1_000_000).toFixed(
          2
        )} USDT)`
      );
      console.log(
        "Locked Collateral:",
        userAccount.lockedCollateral.toString(),
        `(${(userAccount.lockedCollateral.toNumber() / 1_000_000).toFixed(
          2
        )} USDT)`
      );
      console.log(
        "Total PnL:",
        userAccount.totalPnl.toString(),
        `(${(userAccount.totalPnl.toNumber() / 1_000_000).toFixed(2)} USDT)`
      );

      console.log("\n=== Positions ===");

      // Try to fetch positions from index 0 to positionCount
      const positionPromises = [];
      for (let i = 0; i <= userAccount.positionCountTotal; i++) {
        const [positionPda] = anchor.web3.PublicKey.findProgramAddressSync(
          [
            Buffer.from("position"),
            user.publicKey.toBuffer(),
            new anchor.BN(i).toArrayLike(Buffer, "le", 4),
          ],
          program.programId
        );

        positionPromises.push(
          program.account.position
            .fetch(positionPda)
            .then((position) => ({
              index: i,
              pda: positionPda,
              position,
              exists: true,
            }))
            .catch(() => ({
              index: i,
              pda: positionPda,
              position: null,
              exists: false,
            }))
        );
      }

      const positions = await Promise.all(positionPromises);
      const existingPositions = positions; // .filter(p => p.exists);

      if (existingPositions.length === 0) {
        console.log("No positions found");
        return;
      }

      existingPositions.forEach((position) => {
        console.log(position);
      });

      console.log(`\n=== Summary ===`);
      console.log(`Total positions found: ${existingPositions.length}`);
      console.log(`Total positions count: ${existingPositions.length}`);
      console.log(`Expected from user account: ${userAccount.positionCount}`);

      const openPositions = existingPositions.filter(
        (p) => Object.keys(p.position?.status)[0] === "open"
      );
      console.log(`Open positions: ${openPositions.length}`);

      const closedPositions = existingPositions.filter(
        (p) => Object.keys(p.position.status)[0] === "closed"
      );
      console.log(`Closed positions: ${closedPositions.length}`);
    } catch (error) {
      console.error("Error fetching positions:", error);
    }
  });

  it("Add collateral", async () => {
    const [userAccountPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("user"), user.publicKey.toBuffer()],
      program.programId
    );

    const collateralAmount = new anchor.BN(1000000 * 1_000_000); // 10,000 USDT

    try {
      const tx = await program.methods.addCollateral(collateralAmount).rpc();

      console.log("Add collateral tx:", tx);

      const userAccount = await program.account.userAccount.fetch(
        userAccountPda
      );
      console.log("Total collateral:", userAccount.totalCollateral.toString());
      console.log(
        "locked collateral:",
        userAccount.lockedCollateral.toString()
      );
      console.log("user Account", userAccount);
    } catch (error) {
      console.error(" Error:", error);
      throw error;
    }
  });

  it("Open a long position", async () => {
    const symbol = "BTC-USD";
    const size = new anchor.BN(100_000_00); // .1 BTC (8 decimals)
    const entryPrice = new anchor.BN(50000 * 1_000_000); // 50,000 USDT
    const leverage = 10;

    try {
      const tx = await program.methods
        .openPosition(symbol, { long: {} }, size, leverage, entryPrice)
        .rpc(); // PDAs auto-resolved!

      console.log("Open position tx:", tx);

      const [userAccountPda] = anchor.web3.PublicKey.findProgramAddressSync(
        [Buffer.from("user"), user.publicKey.toBuffer()],
        program.programId
      );

      const userAccount = await program.account.userAccount.fetch(userAccountPda);
      const [positionPda] = anchor.web3.PublicKey.findProgramAddressSync(
        [
          Buffer.from("position"),
          user.publicKey.toBuffer(),
          new anchor.BN(userAccount.positionCountTotal - 1).toArrayLike(Buffer, "le", 4),
        ],
        program.programId
      );

      const position = await program.account.position.fetch(positionPda);
      console.log("Position opened:", {
        symbol: position.symbol,
        size: position.size.toString(),
        entryPrice: position.entryPrice.toString(),
        leverage: position.leverage,
        margin: position.margin.toString(),
        liquidationPrice: position.liquidationPrice.toString(),
      });

      expect(position.leverage).to.equal(leverage);
      expect(position.symbol).to.equal(symbol);
    } catch (error) {
      console.error(" Error opening position:", error);
      throw error;
    }
  });

  it("Modify position - increase size", async () => {
    // const positionIndex = ;
    // const [positionPda] = anchor.web3.PublicKey.findProgramAddressSync(
    //   [
    //     Buffer.from("position"),
    //     user.publicKey.toBuffer(),
    //     new anchor.BN(positionIndex).toArrayLike(Buffer, "le", 4),
    //   ],
    //   program.programId
    // );
    const positionPda = new anchor.web3.PublicKey("CZPWM1piyvvm2Byfr9D25ugQjXFSzEHPtsfe6SvKK6vr");

    console.log("PDA:", positionPda.toString());

    const newSize = new anchor.BN(2 * 10_000_000); // 0.2 BTC

    try {
      const tx = await program.methods
        .modifyPosition(newSize, null)
        .accounts({
          position: positionPda,
          // owner: user.publicKey, 
          // userAccount is auto-derived from owner
        })
        .rpc();

      console.log("Modify position tx:", tx);

      const position = await program.account.position.fetch(positionPda);
      console.log("New size:", position.size.toString());
      expect(position.size.toString()).to.equal(newSize.toString());
    } catch (error) {
      console.error(" Error modifying position:", error);
      throw error;
    }
  });

  it("Add margin to position", async () => {
    // const positionIndex = 0;
    // const [positionPda] = anchor.web3.PublicKey.findProgramAddressSync(
    //   [
    //     Buffer.from("position"),
    //     user.publicKey.toBuffer(),
    //     new anchor.BN(positionIndex).toArrayLike(Buffer, "le", 4),
    //   ],
    //   program.programId
    // );

    const positionPda = new anchor.web3.PublicKey("CZPWM1piyvvm2Byfr9D25ugQjXFSzEHPtsfe6SvKK6vr");

    const additionalMargin = new anchor.BN(1000 * 1_000_000); // 1,000 USDT

    try {
      const positionBefore = await program.account.position.fetch(positionPda);
      const marginBefore = positionBefore.margin;

      const tx = await program.methods
        .modifyPosition(null, additionalMargin)
        .accounts({
          position: positionPda,
          // owner: user.publicKey,
        })
        .rpc();

      console.log("Add margin tx:", tx);

      const positionAfter = await program.account.position.fetch(positionPda);
      console.log(
        `Margin: ${marginBefore.toString()} → ${positionAfter.margin.toString()}`
      );
      // expect(positionAfter.margin.gt(marginBefore)).to.be.true;
    } catch (error) {
      console.error(" Error adding margin:", error);
      throw error;
    }
  });

  it("Close position with profit", async () => {
    // const positionIndex = 0;
    // const [positionPda] = anchor.web3.PublicKey.findProgramAddressSync(
    //   [
    //     Buffer.from("position"),
    //     user.publicKey.toBuffer(),
    //     new anchor.BN(positionIndex).toArrayLike(Buffer, "le", 4),
    //   ],
    //   program.programId
    // );

    const positionPda = new anchor.web3.PublicKey("CZPWM1piyvvm2Byfr9D25ugQjXFSzEHPtsfe6SvKK6vr");


    const [userAccountPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("user"), user.publicKey.toBuffer()],
      program.programId
    );

    const finalPrice = new anchor.BN(55000 * 1_000_000); // 55,000 USDT (profit)

    try {
      const userBefore = await program.account.userAccount.fetch(
        userAccountPda
      );

      const tx = await program.methods
        .closePosition(finalPrice)
        .accounts({
          position: positionPda,
          // owner: user.publicKey,
        })
        .rpc();

      console.log("Close position tx:", tx);

      const userAfter = await program.account.userAccount.fetch(userAccountPda);
      console.log("Position closed successfully");
      console.log(`   Total PnL: ${userAfter.totalPnl.toString()}`);
      console.log(`   Position count: ${userAfter.positionCount}`);

      // expect(userAfter.positionCount).to.equal(userBefore.positionCount - 1);
    } catch (error) {
      console.error(" Error closing position:", error);
      throw error;
    }
  });

  it("Open a short position with 50x leverage", async () => {

    const [userAccountPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("user"), user.publicKey.toBuffer()],
      program.programId
    );

    const userAccount = await program.account.userAccount.fetch(userAccountPda);

    const [shortPositionPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [
        Buffer.from("position"),
        user.publicKey.toBuffer(),
        new anchor.BN(userAccount.positionCountTotal).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );

    const symbol = "ETH-USDT";
    const size = new anchor.BN(.1 * 100_000_000); // 10 ETH
    const entryPrice = new anchor.BN(3000 * 1_000_000); // 3,000 USDT
    const leverage = 50;

    try {
      const tx = await program.methods
        .openPosition(
          symbol,
          { short: {} },
          size,
          leverage,
          entryPrice
        )
        .rpc();

      console.log("Open short position tx:", tx);

      const position = await program.account.position.fetch(shortPositionPda);
      console.log("Short position opened:", {
        symbol: position.symbol,
        leverage: position.leverage,
        margin: position.margin.toString(),
      });

      expect(position.leverage).to.equal(leverage);
    } catch (error) {
      console.error(" Error opening short position:", error);
      throw error;
    }
  });
});
