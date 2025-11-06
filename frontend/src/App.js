import React, { useState, useEffect } from 'react';
import { Wallet, TrendingUp, ArrowDownToLine } from 'lucide-react';

function App() {
  const [connected, setConnected] = useState(false);
  const [paymail, setPaymail] = useState('');
  const [balance, setBalance] = useState(null);
  const [depositAmount, setDepositAmount] = useState('');
  const [lockDays, setLockDays] = useState(30);
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState('');

  const connectWallet = async () => {
    const userPaymail = prompt('Enter your Paymail (e.g., yourname@handcash.io):');
    if (!userPaymail) return;
    
    setPaymail(userPaymail);
    setConnected(true);
    setMessage('✅ Connected successfully!');
    await fetchBalance(userPaymail);
  };

  const fetchBalance = async (pm) => {
    try {
      const res = await fetch(`http://localhost:8080/balance/${encodeURIComponent(pm)}`);
      const data = await res.json();
      setBalance(data);
    } catch (error) {
      console.error('Failed to fetch balance:', error);
      setMessage('❌ Failed to fetch balance');
    }
  };

  const handleDeposit = async () => {
    if (!depositAmount || parseFloat(depositAmount) <= 0) {
      setMessage('❌ Please enter a valid amount');
      return;
    }

    setLoading(true);
    setMessage('Creating deposit...');

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
      } else {
        throw new Error('Failed to create deposit');
      }
    } catch (error) {
      console.error('Deposit error:', error);
      setMessage(`❌ Deposit failed: ${error.message}`);
    }
    setLoading(false);
  };

  const formatBSV = (satoshis) => (satoshis / 100000000).toFixed(8);

  return (
    <div style={{ 
      minHeight: '100vh', 
      background: 'linear-gradient(to bottom right, #fff7ed, #fef3c7)', 
      padding: '2rem',
      fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    }}>
      <div style={{ maxWidth: '1200px', margin: '0 auto' }}>
        
        {/* Header */}
        <div style={{ 
          background: 'white', 
          borderRadius: '1rem', 
          boxShadow: '0 10px 25px rgba(0,0,0,0.1)', 
          padding: '2rem', 
          marginBottom: '2rem' 
        }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: '1rem' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '1rem' }}>
              <div style={{ background: 'linear-gradient(135deg, #f97316, #dc2626)', padding: '1rem', borderRadius: '1rem' }}>
                <Wallet style={{ width: '2rem', height: '2rem', color: 'white' }} />
              </div>
              <div>
                <h1 style={{ fontSize: '2rem', fontWeight: 'bold', margin: 0 }}>BSV Bank</h1>
                <p style={{ color: '#666', margin: 0 }}>Algorithmic Banking Platform</p>
              </div>
            </div>
            
            {!connected ? (
              <button 
                onClick={connectWallet} 
                style={{ 
                  background: '#f97316', 
                  color: 'white', 
                  padding: '0.75rem 1.5rem', 
                  borderRadius: '0.75rem', 
                  border: 'none', 
                  cursor: 'pointer', 
                  fontWeight: 'bold',
                  fontSize: '1rem'
                }}
              >
                Connect Wallet
              </button>
            ) : (
              <div style={{ textAlign: 'right' }}>
                <div style={{ fontSize: '0.875rem', color: '#666' }}>Connected</div>
                <div style={{ fontWeight: 'bold', fontFamily: 'monospace', fontSize: '0.875rem' }}>{paymail}</div>
              </div>
            )}
          </div>

          {message && (
            <div style={{ 
              marginTop: '1rem', 
              padding: '1rem', 
              borderRadius: '0.5rem', 
              background: message.includes('❌') ? '#fee2e2' : '#d1fae5', 
              color: message.includes('❌') ? '#991b1b' : '#065f46',
              fontSize: '0.875rem'
            }}>
              {message}
            </div>
          )}
        </div>

        {connected && balance && (
          <>
            {/* Balances */}
            <div style={{ 
              display: 'grid', 
              gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', 
              gap: '1rem', 
              marginBottom: '2rem' 
            }}>
              <div style={{ 
                background: 'linear-gradient(135deg, #10b981, #059669)', 
                color: 'white', 
                padding: '1.5rem', 
                borderRadius: '1rem' 
              }}>
                <div style={{ fontSize: '0.875rem', opacity: 0.9 }}>Balance</div>
                <div style={{ fontSize: '1.5rem', fontWeight: 'bold', margin: '0.5rem 0' }}>
                  {formatBSV(balance.balance_satoshis)} BSV
                </div>
                <div style={{ fontSize: '0.75rem', opacity: 0.75 }}>
                  {balance.active_deposits} active deposits
                </div>
              </div>
              
              <div style={{ 
                background: 'linear-gradient(135deg, #8b5cf6, #7c3aed)', 
                color: 'white', 
                padding: '1.5rem', 
                borderRadius: '1rem' 
              }}>
                <div style={{ fontSize: '0.875rem', opacity: 0.9 }}>Accrued Interest</div>
                <div style={{ fontSize: '1.5rem', fontWeight: 'bold', margin: '0.5rem 0' }}>
                  {formatBSV(balance.accrued_interest_satoshis)} BSV
                </div>
              </div>

              <div style={{ 
                background: 'linear-gradient(135deg, #f59e0b, #d97706)', 
                color: 'white', 
                padding: '1.5rem', 
                borderRadius: '1rem' 
              }}>
                <div style={{ fontSize: '0.875rem', opacity: 0.9 }}>Current APY</div>
                <div style={{ fontSize: '1.5rem', fontWeight: 'bold', margin: '0.5rem 0' }}>
                  {balance.current_apy.toFixed(2)}%
                </div>
              </div>
            </div>

            {/* Deposit Form */}
            <div style={{ 
              background: 'white', 
              borderRadius: '1rem', 
              boxShadow: '0 10px 25px rgba(0,0,0,0.1)', 
              padding: '2rem' 
            }}>
              <h2 style={{ fontSize: '1.5rem', fontWeight: 'bold', marginBottom: '1.5rem', marginTop: 0 }}>
                Create Deposit
              </h2>
              
              <div style={{ marginBottom: '1.5rem' }}>
                <label style={{ 
                  display: 'block', 
                  fontSize: '0.875rem', 
                  fontWeight: '500', 
                  marginBottom: '0.5rem' 
                }}>
                  Amount (BSV)
                </label>
                <input 
                  type="number" 
                  step="0.00000001" 
                  value={depositAmount} 
                  onChange={(e) => setDepositAmount(e.target.value)} 
                  style={{ 
                    width: '100%', 
                    padding: '0.75rem', 
                    border: '2px solid #e5e7eb', 
                    borderRadius: '0.5rem', 
                    fontSize: '1rem' 
                  }} 
                  placeholder="0.001" 
                />
              </div>

              <div style={{ marginBottom: '1.5rem' }}>
                <label style={{ 
                  display: 'block', 
                  fontSize: '0.875rem', 
                  fontWeight: '500', 
                  marginBottom: '0.5rem' 
                }}>
                  Lock Period (Optional)
                </label>
                <div style={{ display: 'flex', gap: '0.5rem', flexWrap: 'wrap' }}>
                  {[0, 7, 30, 90, 365].map(days => (
                    <button 
                      key={days} 
                      onClick={() => setLockDays(days)} 
                      style={{ 
                        flex: '1 1 auto',
                        minWidth: '60px',
                        padding: '0.75rem', 
                        borderRadius: '0.5rem', 
                        border: 'none', 
                        cursor: 'pointer', 
                        fontWeight: '500', 
                        background: lockDays === days ? '#f97316' : '#f3f4f6', 
                        color: lockDays === days ? 'white' : '#374151' 
                      }}
                    >
                      {days === 0 ? 'None' : `${days}d`}
                    </button>
                  ))}
                </div>
                {lockDays > 0 && (
                  <div style={{ fontSize: '0.75rem', color: '#059669', marginTop: '0.5rem' }}>
                    ✓ Earn +{(lockDays / 365 * 2).toFixed(1)}% bonus APY
                  </div>
                )}
              </div>

              <button 
                onClick={handleDeposit} 
                disabled={loading || !depositAmount} 
                style={{ 
                  width: '100%', 
                  background: loading || !depositAmount ? '#9ca3af' : 'linear-gradient(135deg, #f97316, #dc2626)', 
                  color: 'white', 
                  padding: '1rem', 
                  borderRadius: '0.75rem', 
                  border: 'none', 
                  cursor: loading || !depositAmount ? 'not-allowed' : 'pointer', 
                  fontWeight: 'bold', 
                  fontSize: '1rem',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  gap: '0.5rem'
                }}
              >
                <ArrowDownToLine style={{ width: '1.25rem', height: '1.25rem' }} />
                {loading ? 'Processing...' : 'Create Deposit'}
              </button>
            </div>
          </>
        )}

        {!connected && (
          <div style={{ 
            background: 'white', 
            borderRadius: '1rem', 
            boxShadow: '0 10px 25px rgba(0,0,0,0.1)', 
            padding: '3rem', 
            textAlign: 'center' 
          }}>
            <Wallet style={{ width: '5rem', height: '5rem', color: '#d1d5db', margin: '0 auto 1.5rem' }} />
            <h2 style={{ fontSize: '1.5rem', fontWeight: 'bold', marginBottom: '1rem' }}>
              Connect Your Wallet
            </h2>
            <p style={{ color: '#666', marginBottom: '2rem', maxWidth: '400px', margin: '0 auto 2rem' }}>
              Enter your Paymail to start earning algorithmic interest on your BSV deposits
            </p>
            <button 
              onClick={connectWallet} 
              style={{ 
                background: 'linear-gradient(135deg, #f97316, #dc2626)', 
                color: 'white', 
                padding: '1rem 2rem', 
                borderRadius: '0.75rem', 
                border: 'none', 
                cursor: 'pointer', 
                fontWeight: 'bold', 
                fontSize: '1rem' 
              }}
            >
              Connect Wallet
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

export default App;
