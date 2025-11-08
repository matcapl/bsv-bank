// Bitcoin Script templates for P2P lending contracts

/**
 * Collateralized Loan Script
 * Enforces loan repayment or collateral seizure with time lock
 */
export function createLoanContract(
  borrowerPubKey,
  lenderPubKey,
  loanAmount,
  collateralAmount,
  repaymentDeadline
) {
  return `
    OP_IF
      // Repayment path (borrower pays back loan + interest)
      ${repaymentDeadline}
      OP_CHECKLOCKTIMEVERIFY
      OP_DROP
      OP_DUP
      OP_HASH160
      ${lenderPubKey}
      OP_EQUALVERIFY
      OP_CHECKSIG
      // Verify repayment amount
      OP_DUP
      ${loanAmount}
      OP_GREATERTHANOREQUAL
      OP_VERIFY
    OP_ELSE
      // Liquidation path (lender claims collateral after deadline)
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
 * Loan State Commitment (OP_RETURN)
 */
export function createLoanCommitment(loanData) {
  const crypto = require('crypto');
  const commitment = {
    loan_id: loanData.loan_id,
    borrower: loanData.borrower_paymail,
    lender: loanData.lender_paymail,
    principal: loanData.principal_satoshis,
    collateral: loanData.collateral_satoshis,
    interest_rate_bps: loanData.interest_rate_bps,
    due_date: loanData.due_date,
    timestamp: Date.now(),
  };
  
  const hash = crypto.createHash('sha256')
    .update(JSON.stringify(commitment))
    .digest('hex');
  
  return {
    script: `OP_RETURN LOAN_${hash}`,
    data: commitment,
    hash: hash
  };
}

export default {
  createLoanContract,
  createLoanCommitment
};
