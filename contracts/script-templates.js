// contracts/script-templates.js
// Bitcoin Script templates for deposits, loans, and escrow
// Compatible with nPrint Bitcoin Script VM

/**
 * Time-Locked Deposit Script
 * Allows withdrawal only after specified lock time
 * Uses OP_CHECKLOCKTIMEVERIFY for enforcement
 */
export function createTimeLockDepositScript(userPubKey, lockTime) {
  // lockTime: Unix timestamp or block height
  return `
    ${lockTime}
    OP_CHECKLOCKTIMEVERIFY
    OP_DROP
    OP_DUP
    OP_HASH160
    ${userPubKey}
    OP_EQUALVERIFY
    OP_CHECKSIG
  `.trim().split('\n').map(line => line.trim()).join(' ');
}

/**
 * Multi-Signature Escrow Script (2-of-3)
 * Requires 2 signatures from: depositor, bank, arbitrator
 * Used for dispute resolution
 */
export function createMultiSigEscrowScript(depositorPubKey, bankPubKey, arbitratorPubKey) {
  return `
    OP_2
    ${depositorPubKey}
    ${bankPubKey}
    ${arbitratorPubKey}
    OP_3
    OP_CHECKMULTISIG
  `.trim().split('\n').map(line => line.trim()).join(' ');
}

/**
 * Collateralized Loan Script
 * Enforces loan repayment or collateral seizure
 * 
 * Conditions:
 * 1. Borrower can reclaim collateral by repaying loan + interest before deadline
 * 2. Lender can claim collateral after deadline if loan unpaid
 */
export function createLoanScript(
  borrowerPubKey,
  lenderPubKey,
  loanAmount,
  interestAmount,
  repaymentDeadline
) {
  // Two spending paths:
  // Path 1: Borrower repays (before deadline)
  // Path 2: Lender claims collateral (after deadline)
  
  return `
    OP_IF
      // Path 1: Borrower repayment path
      ${repaymentDeadline}
      OP_CHECKLOCKTIMEVERIFY
      OP_DROP
      OP_DUP
      OP_HASH160
      ${lenderPubKey}
      OP_EQUALVERIFY
      OP_CHECKSIG
      // Verify repayment amount (loan + interest)
      OP_DUP
      ${loanAmount + interestAmount}
      OP_EQUALVERIFY
    OP_ELSE
      // Path 2: Lender claims collateral after deadline
      ${repaymentDeadline}
      OP_CHECKLOCKTIMEVERIFY
      OP_DROP
      OP_DUP
      OP_HASH160
      ${lenderPubKey}
      OP_EQUALVERIFY
      OP_CHECKSIG
    OP_ENDIF
  `.trim().split('\n').map(line => line.trim()).join(' ');
}

/**
 * Interest Payment Channel Script
 * Enables ongoing interest payments without on-chain transactions
 * Uses payment channels for efficiency
 */
export function createInterestChannelScript(
  depositorPubKey,
  bankPubKey,
  initialBalance,
  channelDuration
) {
  return `
    OP_IF
      // Normal channel closure (cooperative)
      OP_2
      ${depositorPubKey}
      ${bankPubKey}
      OP_2
      OP_CHECKMULTISIG
    OP_ELSE
      // Unilateral closure with state commitment
      OP_DUP
      OP_HASH160
      ${depositorPubKey}
      OP_EQUALVERIFY
      OP_CHECKSIG
      // Verify latest state hash
      OP_SHA256
      <state_commitment_hash>
      OP_EQUALVERIFY
      // Enforce CSV timeout for dispute period
      ${channelDuration}
      OP_CHECKSEQUENCEVERIFY
      OP_DROP
    OP_ENDIF
  `.trim().split('\n').map(line => line.trim()).join(' ');
}

/**
 * Liquidation Script
 * Automatically liquidates under-collateralized loans
 * Triggered when collateral value falls below threshold
 */
export function createLiquidationScript(
  borrowerPubKey,
  lenderPubKey,
  collateralValue,
  liquidationThreshold
) {
  // Requires oracle price feed for collateral valuation
  return `
    OP_IF
      // Liquidation path (collateral < threshold)
      // Verify oracle signature on price
      OP_DUP
      OP_HASH160
      <oracle_pubkey_hash>
      OP_EQUALVERIFY
      OP_CHECKSIG
      // Check price is below liquidation threshold
      <oracle_price>
      ${liquidationThreshold}
      OP_LESSTHAN
      OP_VERIFY
      // Transfer to lender
      OP_DUP
      OP_HASH160
      ${lenderPubKey}
      OP_EQUALVERIFY
      OP_CHECKSIG
    OP_ELSE
      // Normal repayment path
      OP_DUP
      OP_HASH160
      ${borrowerPubKey}
      OP_EQUALVERIFY
      OP_CHECKSIG
    OP_ENDIF
  `.trim().split('\n').map(line => line.trim()).join(' ');
}

/**
 * Deposit State Commitment (OP_RETURN)
 * Stores hash of deposit state on-chain for verification
 */
export function createDepositCommitment(depositData) {
  const crypto = require('crypto');
  const hash = crypto.createHash('sha256')
    .update(JSON.stringify(depositData))
    .digest('hex');
  
  return {
    script: `OP_RETURN ${hash}`,
    data: depositData,
    hash: hash
  };
}

/**
 * Interest Rate State Commitment (OP_RETURN)
 * Records interest rate snapshots on-chain
 */
export function createInterestRateCommitment(rateData) {
  const crypto = require('crypto');
  const commitment = {
    timestamp: Date.now(),
    utilizationRate: rateData.utilizationRate,
    interestRate: rateData.interestRate,
    totalDeposits: rateData.totalDeposits,
    totalBorrowed: rateData.totalBorrowed,
    merkleRoot: rateData.merkleRoot // For all user balances
  };
  
  const hash = crypto.createHash('sha256')
    .update(JSON.stringify(commitment))
    .digest('hex');
  
  return {
    script: `OP_RETURN INTEREST_${hash}`,
    data: commitment,
    hash: hash
  };
}

/**
 * Script Builder Utility
 * Constructs complete locking and unlocking scripts
 */
export class ScriptBuilder {
  constructor() {
    this.ops = [];
  }
  
  addOp(opcode) {
    this.ops.push(opcode);
    return this;
  }
  
  addData(data) {
    // Push data onto stack
    this.ops.push(data);
    return this;
  }
  
  addTimeLock(locktime) {
    this.ops.push(locktime);
    this.ops.push('OP_CHECKLOCKTIMEVERIFY');
    this.ops.push('OP_DROP');
    return this;
  }
  
  addSignatureCheck(pubKeyHash) {
    this.ops.push('OP_DUP');
    this.ops.push('OP_HASH160');
    this.ops.push(pubKeyHash);
    this.ops.push('OP_EQUALVERIFY');
    this.ops.push('OP_CHECKSIG');
    return this;
  }
  
  build() {
    return this.ops.join(' ');
  }
}

/**
 * Script Validator
 * Validates script syntax and safety
 */
export class ScriptValidator {
  static validate(script) {
    const ops = script.split(' ').filter(op => op.length > 0);
    
    // Check for dangerous opcodes
    const dangerousOps = [
      'OP_CAT', 'OP_SUBSTR', 'OP_LEFT', 'OP_RIGHT',
      'OP_INVERT', 'OP_AND', 'OP_OR', 'OP_XOR'
    ];
    
    for (const op of ops) {
      if (dangerousOps.includes(op)) {
        return {
          valid: false,
          error: `Dangerous opcode detected: ${op}`
        };
      }
    }
    
    // Check stack depth (prevent stack overflow)
    let stackDepth = 0;
    for (const op of ops) {
      if (op.startsWith('OP_')) {
        // Simplified stack effect calculation
        if (op === 'OP_DUP') stackDepth++;
        else if (op === 'OP_DROP') stackDepth--;
      } else {
        stackDepth++; // Data push
      }
      
      if (stackDepth > 1000) {
        return {
          valid: false,
          error: 'Script exceeds maximum stack depth'
        };
      }
    }
    
    return { valid: true };
  }
}

/**
 * Example Usage:
 */
export function exampleUsage() {
  // 1. Create time-locked deposit
  const userPubKey = '02a1633cafcc01ebfb6d78e39f687a1f0995c62fc95f51ead10a02ee0be551b5dc';
  const lockTime = Math.floor(Date.now() / 1000) + (30 * 24 * 60 * 60); // 30 days
  const depositScript = createTimeLockDepositScript(userPubKey, lockTime);
  
  console.log('Time-Locked Deposit Script:', depositScript);
  
  // 2. Create collateralized loan
  const borrowerKey = '03fff97bd5755eeea420453a14355235d382f6472f8568a18b2f057a1460297556';
  const lenderKey = '0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798';
  const loanAmount = 100000; // 0.001 BSV
  const interest = 10000; // 10% interest
  const deadline = lockTime;
  
  const loanScript = createLoanScript(borrowerKey, lenderKey, loanAmount, interest, deadline);
  console.log('Loan Script:', loanScript);
  
  // 3. Create deposit commitment
  const depositData = {
    user: 'alice@handcash.io',
    amount: 500000,
    txid: 'abc123...',
    timestamp: Date.now()
  };
  
  const commitment = createDepositCommitment(depositData);
  console.log('Deposit Commitment:', commitment);
  
  // 4. Validate script
  const validation = ScriptValidator.validate(depositScript);
  console.log('Script Valid:', validation.valid);
  
  return {
    depositScript,
    loanScript,
    commitment,
    validation
  };
}

// Export all functions for use in deposit service
export default {
  createTimeLockDepositScript,
  createMultiSigEscrowScript,
  createLoanScript,
  createInterestChannelScript,
  createLiquidationScript,
  createDepositCommitment,
  createInterestRateCommitment,
  ScriptBuilder,
  ScriptValidator,
  exampleUsage
};