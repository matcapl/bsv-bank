import React, { useState } from 'react';
import { Wallet, TrendingUp, ArrowDownToLine, Coins, AlertCircle } from 'lucide-react';

function App() {
  const [connected, setConnected] = useState(false);
  const [paymail, setPaymail] = useState('');
  const [balance, setBalance] = useState(null);
  const [depositAmount, setDepositAmount] = useState('');
  const [lockDays, setLockDays] = useState(30);
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState('');
  const [activeTab, setActiveTab] = useState('deposit');
  
  // Lending state
  const [loanAmount, setLoanAmount] = useState('');
  const [collateral, setCollateral] = useState('');
  const [loanDuration, setLoanDuration] = useState(30);
  const [interestRate, setInterestRate] = useState(1000); // 10% in bps
  const [availableLoans, setAvailableLoans] = useState([]);

  const connectWallet = async () => {
    const userPaymail = prompt('Enter your Paymail (e.g., yourname@handcash.io):');
    if (!userPaymail || !userPaymail.includes('@')) {
      setMessage('❌ Invalid paymail format');
      return;
    }
    
    setPaymail(userPaymail);
    setLoading(true);
    
    try {
      await fetchBalance(userPaymail);
      setConnected(true);
      setMessage('✅ Connected successfully!');
    } catch (error) {
      setMessage('❌ Connection failed: ' + error.message);
    }
    setLoading(false);
  };

  const fetchBalance = async (pm) => {
    try {
      const res = await fetch(`http://localhost:8080/balance/${encodeURIComponent(pm)}`);
      if (!res.ok) {
        throw new Error(`HTTP ${res.status}`);
      }
      const data = await res.json();
      setBalance(data);
    } catch (error) {
      console.error('Failed to fetch balance:', error);
      // Create empty balance for new users
      setBalance({
        balance_satoshis: 0,
        accrued_interest_satoshis: 0,
        total_available_satoshis: 0,
        current_apy: 7.0,
        active_deposits: 0
      });
    }
  };

  const fetchAvailableLoans = async () => {
    try {
      const res = await fetch('http://localhost:8082/loans/available');
      const data = await res.json();
      setAvailableLoans(data);
    } catch (error) {
      console.error('Failed to fetch loans:', error);
    }
  };

  const handleDeposit = async () => {
    if (!depositAmount || parseFloat(depositAmount) <= 0) {
      setMessage('❌ Please enter a valid amount');
      return;
    }

    setLoading(true);
    try {
      const mockTxid = Array(64).fill(0).map(() => 
        Math.floor(Math.random() * 16).toString(16)
      ).join('');

      const response = await fetch('http://localhost:8080/deposits', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          user_paymail: paymail,
          amount_satoshis: Math.floor(parseFloat(depositAmount) * 100000000),
          txid: mockTxid,
          lock_duration_days: lockDays > 0 ? lockDays : null,
        }),
      });

      if (response.ok) {
        const result = await response.json();
        setMessage(`✅ Deposit created! ID: ${result.deposit_id.substring(0, 8)}...`);
        setDepositAmount('');
        await fetchBalance(paymail);
      }
    } catch (error) {
      setMessage(`❌ Deposit failed: ${error.message}`);
    }
    setLoading(false);
  };

  const handleLoanRequest = async () => {
    if (!loanAmount || !collateral) {
      setMessage('❌ Please enter loan amount and collateral');
      return;
    }

    const amountSats = Math.floor(parseFloat(loanAmount) * 100000000);
    const collateralSats = Math.floor(parseFloat(collateral) * 100000000);

    if (collateralSats < amountSats * 1.5) {
      setMessage('❌ Collateral must be at least 150% of loan amount');
      return;
    }

    setLoading(true);
    try {
      const response = await fetch('http://localhost:8082/loans/request', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          borrower_paymail: paymail,
          amount_satoshis: amountSats,
          collateral_satoshis: collateralSats,
          duration_days: loanDuration,
          interest_rate_bps: interestRate,
        }),
      });

      if (response.ok) {
        const result = await response.json();
        setMessage(`✅ Loan request created! Total repayment: ${(result.total_repayment_satoshis / 100000000).toFixed(8)} BSV`);
        setLoanAmount('');
        setCollateral('');
        fetchAvailableLoans();
      } else {
        const error = await response.json();
        setMessage(`❌ ${error.error}`);
      }
    } catch (error) {
      setMessage(`❌ Failed: ${error.message}`);
    }
    setLoading(false);
  };

  const handleFundLoan = async (loanId) => {
    setLoading(true);
    try {
      const response = await fetch(`http://localhost:8082/loans/${loanId}/fund`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ lender_paymail: paymail }),
      });

      if (response.ok) {
        setMessage('✅ Loan funded successfully!');
        fetchAvailableLoans();
      }
    } catch (error) {
      setMessage(`❌ Failed: ${error.message}`);
    }
    setLoading(false);
  };

  const formatBSV = (satoshis) => (satoshis / 100000000).toFixed(8);

  React.useEffect(() => {
    if (activeTab === 'lend') {
      fetchAvailableLoans();
    }
  }, [activeTab]);

  return (
    <div style={{ minHeight: '100vh', background: 'linear-gradient(to bottom right, #fff7ed, #fef3c7)', padding: '2rem', fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif' }}>
      <div style={{ maxWidth: '1200px', margin: '0 auto' }}>
        
        {/* Header */}
        <div style={{ background: 'white', borderRadius: '1rem', boxShadow: '0 10px 25px rgba(0,0,0,0.1)', padding: '2rem', marginBottom: '2rem' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: '1rem' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '1rem' }}>
              <div style={{ background: 'linear-gradient(135deg, #f97316, #dc2626)', padding: '1rem', borderRadius: '1rem' }}>
                <Wallet style={{ width: '2rem', height: '2rem', color: 'white' }} />
              </div>
              <div>
                <h1 style={{ fontSize: '2rem', fontWeight: 'bold', margin: 0 }}>BSV Bank</h1>
                <p style={{ color: '#666', margin: 0 }}>Deposits • Lending • Interest</p>
              </div>
            </div>
            
            {!connected ? (
              <button onClick={connectWallet} disabled={loading} style={{ background: '#f97316', color: 'white', padding: '0.75rem 1.5rem', borderRadius: '0.75rem', border: 'none', cursor: loading ? 'not-allowed' : 'pointer', fontWeight: 'bold', fontSize: '1rem', opacity: loading ? 0.5 : 1 }}>
                {loading ? 'Connecting...' : 'Connect Wallet'}
              </button>
            ) : (
              <div style={{ textAlign: 'right' }}>
                <div style={{ fontSize: '0.875rem', color: '#666' }}>Connected</div>
                <div style={{ fontWeight: 'bold', fontFamily: 'monospace', fontSize: '0.875rem' }}>{paymail}</div>
              </div>
            )}
          </div>

          {message && (
            <div style={{ marginTop: '1rem', padding: '1rem', borderRadius: '0.5rem', background: message.includes('❌') ? '#fee2e2' : '#d1fae5', color: message.includes('❌') ? '#991b1b' : '#065f46', fontSize: '0.875rem' }}>
              {message}
            </div>
          )}
        </div>

        {connected && balance && (
          <>
            {/* Balance Cards */}
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: '1rem', marginBottom: '2rem' }}>
              <div style={{ background: 'linear-gradient(135deg, #10b981, #059669)', color: 'white', padding: '1.5rem', borderRadius: '1rem' }}>
                <div style={{ fontSize: '0.875rem', opacity: 0.9 }}>Balance</div>
                <div style={{ fontSize: '1.5rem', fontWeight: 'bold', margin: '0.5rem 0' }}>{formatBSV(balance.balance_satoshis)} BSV</div>
              </div>
              <div style={{ background: 'linear-gradient(135deg, #8b5cf6, #7c3aed)', color: 'white', padding: '1.5rem', borderRadius: '1rem' }}>
                <div style={{ fontSize: '0.875rem', opacity: 0.9 }}>Interest</div>
                <div style={{ fontSize: '1.5rem', fontWeight: 'bold', margin: '0.5rem 0' }}>{formatBSV(balance.accrued_interest_satoshis)} BSV</div>
              </div>
              <div style={{ background: 'linear-gradient(135deg, #f59e0b, #d97706)', color: 'white', padding: '1.5rem', borderRadius: '1rem' }}>
                <div style={{ fontSize: '0.875rem', opacity: 0.9 }}>APY</div>
                <div style={{ fontSize: '1.5rem', fontWeight: 'bold', margin: '0.5rem 0' }}>{balance.current_apy.toFixed(2)}%</div>
              </div>
            </div>

            {/* Tabs */}
            <div style={{ background: 'white', borderRadius: '1rem', boxShadow: '0 10px 25px rgba(0,0,0,0.1)', padding: '2rem', marginBottom: '2rem' }}>
              <div style={{ display: 'flex', gap: '1rem', marginBottom: '2rem', borderBottom: '2px solid #e5e7eb' }}>
                {['deposit', 'borrow', 'lend'].map(tab => (
                  <button key={tab} onClick={() => setActiveTab(tab)} style={{ padding: '0.75rem 1.5rem', border: 'none', background: 'none', cursor: 'pointer', fontWeight: '600', color: activeTab === tab ? '#f97316' : '#6b7280', borderBottom: activeTab === tab ? '3px solid #f97316' : 'none', marginBottom: '-2px' }}>
                    {tab.charAt(0).toUpperCase() + tab.slice(1)}
                  </button>
                ))}
              </div>

              {activeTab === 'deposit' && (
                <div style={{ space: '1.5rem' }}>
                  <div style={{ marginBottom: '1.5rem' }}>
                    <label style={{ display: 'block', fontSize: '0.875rem', fontWeight: '500', marginBottom: '0.5rem' }}>Amount (BSV)</label>
                    <input type="number" step="0.00000001" value={depositAmount} onChange={(e) => setDepositAmount(e.target.value)} style={{ width: '100%', padding: '0.75rem', border: '2px solid #e5e7eb', borderRadius: '0.5rem', fontSize: '1rem' }} placeholder="0.001" />
                  </div>
                  <div style={{ marginBottom: '1.5rem' }}>
                    <label style={{ display: 'block', fontSize: '0.875rem', fontWeight: '500', marginBottom: '0.5rem' }}>Lock Period</label>
                    <div style={{ display: 'flex', gap: '0.5rem', flexWrap: 'wrap' }}>
                      {[0, 7, 30, 90, 365].map(days => (
                        <button key={days} onClick={() => setLockDays(days)} style={{ flex: '1 1 auto', minWidth: '60px', padding: '0.75rem', borderRadius: '0.5rem', border: 'none', cursor: 'pointer', fontWeight: '500', background: lockDays === days ? '#f97316' : '#f3f4f6', color: lockDays === days ? 'white' : '#374151' }}>
                          {days === 0 ? 'None' : `${days}d`}
                        </button>
                      ))}
                    </div>
                  </div>
                  <button onClick={handleDeposit} disabled={loading || !depositAmount} style={{ width: '100%', background: loading || !depositAmount ? '#9ca3af' : 'linear-gradient(135deg, #f97316, #dc2626)', color: 'white', padding: '1rem', borderRadius: '0.75rem', border: 'none', cursor: loading || !depositAmount ? 'not-allowed' : 'pointer', fontWeight: 'bold', fontSize: '1rem', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '0.5rem' }}>
                    <ArrowDownToLine style={{ width: '1.25rem', height: '1.25rem' }} />
                    {loading ? 'Processing...' : 'Create Deposit'}
                  </button>
                </div>
              )}

              {activeTab === 'borrow' && (
                <div>
                  <div style={{ background: '#fef3c7', padding: '1rem', borderRadius: '0.5rem', marginBottom: '1.5rem', display: 'flex', gap: '0.75rem' }}>
                    <AlertCircle style={{ width: '1.25rem', height: '1.25rem', color: '#d97706', flexShrink: 0 }} />
                    <div style={{ fontSize: '0.875rem', color: '#92400e' }}>
                      Minimum 150% collateral required. Your collateral is locked until loan repayment.
                    </div>
                  </div>
                  <div style={{ marginBottom: '1.5rem' }}>
                    <label style={{ display: 'block', fontSize: '0.875rem', fontWeight: '500', marginBottom: '0.5rem' }}>Loan Amount (BSV)</label>
                    <input type="number" step="0.00000001" value={loanAmount} onChange={(e) => setLoanAmount(e.target.value)} style={{ width: '100%', padding: '0.75rem', border: '2px solid #e5e7eb', borderRadius: '0.5rem', fontSize: '1rem' }} placeholder="0.001" />
                  </div>
                  <div style={{ marginBottom: '1.5rem' }}>
                    <label style={{ display: 'block', fontSize: '0.875rem', fontWeight: '500', marginBottom: '0.5rem' }}>Collateral (BSV) - Min: {loanAmount ? (parseFloat(loanAmount) * 1.5).toFixed(8) : '0'}</label>
                    <input type="number" step="0.00000001" value={collateral} onChange={(e) => setCollateral(e.target.value)} style={{ width: '100%', padding: '0.75rem', border: '2px solid #e5e7eb', borderRadius: '0.5rem', fontSize: '1rem' }} placeholder="0.0015" />
                  </div>
                  <button onClick={handleLoanRequest} disabled={loading} style={{ width: '100%', background: loading ? '#9ca3af' : 'linear-gradient(135deg, #3b82f6, #1d4ed8)', color: 'white', padding: '1rem', borderRadius: '0.75rem', border: 'none', cursor: loading ? 'not-allowed' : 'pointer', fontWeight: 'bold', fontSize: '1rem' }}>
                    {loading ? 'Processing...' : 'Request Loan'}
                  </button>
                </div>
              )}

              {activeTab === 'lend' && (
                <div>
                  <h3 style={{ fontSize: '1.25rem', fontWeight: 'bold', marginBottom: '1rem' }}>Available Loan Requests</h3>
                  {availableLoans.length === 0 ? (
                    <div style={{ textAlign: 'center', padding: '3rem', color: '#6b7280' }}>
                      <Coins style={{ width: '3rem', height: '3rem', margin: '0 auto 1rem', color: '#d1d5db' }} />
                      <p>No loan requests available</p>
                    </div>
                  ) : (
                    <div style={{ display: 'grid', gap: '1rem' }}>
                      {availableLoans.map(loan => (
                        <div key={loan.loan_id} style={{ border: '2px solid #e5e7eb', borderRadius: '0.75rem', padding: '1.5rem' }}>
                          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'start', marginBottom: '1rem' }}>
                            <div>
                              <div style={{ fontSize: '0.875rem', color: '#6b7280' }}>Borrower</div>
                              <div style={{ fontFamily: 'monospace', fontSize: '0.875rem', fontWeight: '500' }}>{loan.borrower}</div>
                            </div>
                            <div style={{ textAlign: 'right' }}>
                              <div style={{ fontSize: '1.25rem', fontWeight: 'bold' }}>{formatBSV(loan.amount)} BSV</div>
                              <div style={{ fontSize: '0.75rem', color: '#059669' }}>{loan.interest_rate_percent.toFixed(1)}% APR</div>
                            </div>
                          </div>
                          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '1rem', marginBottom: '1rem', fontSize: '0.875rem' }}>
                            <div>
                              <span style={{ color: '#6b7280' }}>Collateral: </span>
                              <span style={{ fontWeight: '500' }}>{formatBSV(loan.collateral)} BSV ({(loan.collateral_ratio * 100).toFixed(0)}%)</span>
                            </div>
                            <div>
                              <span style={{ color: '#6b7280' }}>Due: </span>
                              <span style={{ fontWeight: '500' }}>{new Date(loan.due_date).toLocaleDateString()}</span>
                            </div>
                          </div>
                          <button onClick={() => handleFundLoan(loan.loan_id)} disabled={loading} style={{ width: '100%', background: 'linear-gradient(135deg, #10b981, #059669)', color: 'white', padding: '0.75rem', borderRadius: '0.5rem', border: 'none', cursor: loading ? 'not-allowed' : 'pointer', fontWeight: '600' }}>
                            Fund This Loan
                          </button>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}

export default App;
