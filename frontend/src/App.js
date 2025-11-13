import React, { useState, useEffect } from 'react';
import { Wallet, TrendingUp, ArrowDownToLine, Coins, AlertCircle } from 'lucide-react';
import LoanHistory from './components/LoanHistory';
import PaymentChannels from './components/PaymentChannels';

function App() {
  const [connected, setConnected] = useState(false);
  const [paymail, setPaymail] = useState('');
  const [balance, setBalance] = useState(null);
  const [depositAmount, setDepositAmount] = useState('');
  const [lockDays, setLockDays] = useState(0);
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState('');
  const [activeTab, setActiveTab] = useState('deposit');
  const [loanAmount, setLoanAmount] = useState('');
  const [collateral, setCollateral] = useState('');
  const [loanDuration, setLoanDuration] = useState(30);
  const [interestRate, setInterestRate] = useState(1000);
  const [availableLoans, setAvailableLoans] = useState([]);
  const [currentView, setCurrentView] = useState('dashboard');

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
      setMessage('❌ Connection failed');
    } finally {
      setLoading(false);
    }
  };

  const fetchBalance = async (userPaymail) => {
    try {
      const response = await fetch(`http://localhost:8080/balance/${userPaymail}`);
      const data = await response.json();
      setBalance(data);
    } catch (error) {
      console.error('Failed to fetch balance:', error);
      setBalance({ balance_satoshis: 0, accrued_interest_satoshis: 0, total_available_satoshis: 0, current_apy: 7.0, active_deposits: 0 });
    }
  };

  const fetchAvailableLoans = async () => {
    try {
      const response = await fetch('http://localhost:8082/loans/available');
      const data = await response.json();
      const formatted = data.map(loan => ({
        loan_id: loan.id,
        borrower: loan.borrower_paymail,
        amount: loan.principal_satoshis,
        collateral: loan.collateral_satoshis,
        collateral_ratio: loan.collateral_satoshis / loan.principal_satoshis,
        interest_rate_percent: (loan.interest_rate_bps / 100),
        due_date: loan.due_date,
      }));
      setAvailableLoans(formatted);
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
      const mockTxid = Array(64).fill(0).map(() => Math.floor(Math.random() * 16).toString(16)).join('');
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
        setMessage('❌ Deposit failed');
      }
    } catch (error) {
      setMessage('❌ Network error');
    } finally {
      setLoading(false);
    }
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
      } else {
        setMessage('❌ Loan request failed');
      }
    } catch (error) {
      setMessage('❌ Network error');
    } finally {
      setLoading(false);
    }
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
      } else {
        setMessage('❌ Failed to fund loan');
      }
    } catch (error) {
      setMessage('❌ Network error');
    } finally {
      setLoading(false);
    }
  };

  const formatBSV = (satoshis) => (satoshis / 100000000).toFixed(8);

  useEffect(() => {
    if (activeTab === 'lend') {
      fetchAvailableLoans();
    }
  }, [activeTab]);

  return (
    <div className="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100 p-6 font-sans">
      <header className="flex justify-between items-center mb-8">
        <h1 className="text-3xl font-bold text-gray-800 flex items-center gap-2">
          <Coins className="w-8 h-8 text-orange-500" />
          BSV Bank
          <span className="text-xl font-normal text-gray-600 ml-2">Deposits • Lending • Interest</span>
        </h1>
        {!connected ? (
          <button
            onClick={connectWallet}
            disabled={loading}
            className="bg-gradient-to-r from-orange-500 to-orange-600 text-white px-6 py-3 rounded-lg font-semibold flex items-center gap-2 hover:from-orange-600 hover:to-orange-700 transition-colors disabled:opacity-50"
          >
            <Wallet className="w-5 h-5" />
            {loading ? 'Connecting...' : 'Connect Wallet'}
          </button>
        ) : (
          <div className="text-green-600 font-semibold flex items-center gap-2">
            <AlertCircle className="w-5 h-5" />
            Connected <span className="bg-green-100 px-3 py-1 rounded-full">{paymail}</span>
          </div>
        )}
      </header>

      {message && (
        <div className="mb-6 p-4 rounded-lg bg-orange-50 border border-orange-200 text-orange-800 flex items-center gap-2">
          <AlertCircle className="w-5 h-5 flex-shrink-0" />
          {message}
        </div>
      )}

      {connected && (
        <nav className="mb-4 flex gap-4">
          <button
            onClick={() => setCurrentView('dashboard')}
            style={{
              padding: '0.75rem 1.5rem',
              background: currentView === 'dashboard' ? '#f97316' : '#f3f4f6',
              color: currentView === 'dashboard' ? 'white' : '#374151',
              borderRadius: '0.5rem',
              border: 'none',
              cursor: 'pointer',
              fontWeight: '600',
            }}
          >
            Dashboard
          </button>
          <button
            onClick={() => setCurrentView('history')}
            style={{
              padding: '0.75rem 1.5rem',
              background: currentView === 'history' ? '#f97316' : '#f3f4f6',
              color: currentView === 'history' ? 'white' : '#374151',
              borderRadius: '0.5rem',
              border: 'none',
              cursor: 'pointer',
              fontWeight: '600',
            }}
          >
            Loan History
          </button>
          <button
            onClick={() => setCurrentView('channels')}
            style={{
              padding: '0.75rem 1.5rem',
              background: currentView === 'channels' ? '#f97316' : '#f3f4f6',
              color: currentView === 'channels' ? 'white' : '#374151',
              borderRadius: '0.5rem',
              border: 'none',
              cursor: 'pointer',
              fontWeight: '600',
            }}
          >
            Payment Channels
          </button>
        </nav>
      )}

      {connected && balance && currentView === 'dashboard' && (
        <>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-8">
            <div className="bg-white p-6 rounded-xl shadow-sm border border-gray-100">
              <div className="flex items-center gap-2 mb-2">
                <Wallet className="w-5 h-5 text-blue-500" />
                <h3 className="font-semibold text-gray-700">Balance</h3>
              </div>
              <p className="text-2xl font-bold text-gray-900">{formatBSV(balance.balance_satoshis)} BSV</p>
            </div>
            <div className="bg-white p-6 rounded-xl shadow-sm border border-gray-100">
              <div className="flex items-center gap-2 mb-2">
                <TrendingUp className="w-5 h-5 text-green-500" />
                <h3 className="font-semibold text-gray-700">Interest</h3>
              </div>
              <p className="text-2xl font-bold text-gray-900">{formatBSV(balance.accrued_interest_satoshis)} BSV</p>
            </div>
            <div className="bg-white p-6 rounded-xl shadow-sm border border-gray-100">
              <div className="flex items-center gap-2 mb-2">
                <ArrowDownToLine className="w-5 h-5 text-purple-500" />
                <h3 className="font-semibold text-gray-700">APY</h3>
              </div>
              <p className="text-2xl font-bold text-gray-900">{balance.current_apy.toFixed(2)}%</p>
            </div>
          </div>

          <div className="bg-white rounded-xl shadow-sm overflow-hidden">
            <div className="border-b border-gray-200 flex">
              {['deposit', 'borrow', 'lend'].map(tab => (
                <button
                  key={tab}
                  onClick={() => setActiveTab(tab)}
                  style={{
                    padding: '0.75rem 1.5rem',
                    border: 'none',
                    background: 'none',
                    cursor: 'pointer',
                    fontWeight: '600',
                    color: activeTab === tab ? '#f97316' : '#6b7280',
                    borderBottom: activeTab === tab ? '3px solid #f97316' : 'none',
                    marginBottom: '-2px'
                  }}
                >
                  {tab.charAt(0).toUpperCase() + tab.slice(1)}
                </button>
              ))}
            </div>
            <div className="p-6">
              {activeTab === 'deposit' && (
                <div className="space-y-6">
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">Amount (BSV)</label>
                    <input
                      type="number"
                      value={depositAmount}
                      onChange={e => setDepositAmount(e.target.value)}
                      style={{ width: '100%', padding: '0.75rem', border: '2px solid #e5e7eb', borderRadius: '0.5rem', fontSize: '1rem' }}
                      placeholder="0.001"
                    />
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">Lock Period</label>
                    <div className="flex gap-2">
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
                  </div>
                  <button
                    onClick={handleDeposit}
                    disabled={loading}
                    style={{
                      width: '100%',
                      background: 'linear-gradient(135deg, #f97316, #ea580c)',
                      color: 'white',
                      padding: '0.75rem',
                      borderRadius: '0.5rem',
                      border: 'none',
                      cursor: loading ? 'not-allowed' : 'pointer',
                      fontWeight: '600'
                    }}
                  >
                    {loading ? 'Processing...' : 'Create Deposit'}
                  </button>
                </div>
              )}
              {activeTab === 'borrow' && (
                <div className="space-y-6">
                  <p className="text-sm text-gray-600 flex items-center gap-2">
                    <AlertCircle className="w-4 h-4" />
                    Minimum 150% collateral required. Your collateral is locked until loan repayment.
                  </p>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">Loan Amount (BSV)</label>
                    <input
                      type="number"
                      value={loanAmount}
                      onChange={e => setLoanAmount(e.target.value)}
                      style={{ width: '100%', padding: '0.75rem', border: '2px solid #e5e7eb', borderRadius: '0.5rem', fontSize: '1rem' }}
                      placeholder="0.001"
                    />
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Collateral (BSV) - Min: {loanAmount ? (parseFloat(loanAmount) * 1.5).toFixed(8) : '0'}
                    </label>
                    <input
                      type="number"
                      value={collateral}
                      onChange={e => setCollateral(e.target.value)}
                      style={{ width: '100%', padding: '0.75rem', border: '2px solid #e5e7eb', borderRadius: '0.5rem', fontSize: '1rem' }}
                      placeholder="0.0015"
                    />
                  </div>
                  <button
                    onClick={handleLoanRequest}
                    disabled={loading}
                    style={{
                      width: '100%',
                      background: 'linear-gradient(135deg, #f97316, #ea580c)',
                      color: 'white',
                      padding: '0.75rem',
                      borderRadius: '0.5rem',
                      border: 'none',
                      cursor: loading ? 'not-allowed' : 'pointer',
                      fontWeight: '600'
                    }}
                  >
                    {loading ? 'Processing...' : 'Request Loan'}
                  </button>
                </div>
              )}
              {activeTab === 'lend' && (
                <div className="space-y-6">
                  <h3 className="text-lg font-semibold text-gray-800">Available Loan Requests</h3>
                  {availableLoans.length === 0 ? (
                    <p className="text-gray-500">No loan requests available</p>
                  ) : (
                    <div className="space-y-4">
                      {availableLoans.map(loan => (
                        <div key={loan.loan_id} className="bg-gray-50 p-4 rounded-lg space-y-2">
                          <div className="flex justify-between">
                            <span className="font-medium">Borrower</span>
                            <span>{loan.borrower}</span>
                          </div>
                          <div className="flex justify-between">
                            <span className="font-medium">Amount</span>
                            <span>{formatBSV(loan.amount)} BSV</span>
                          </div>
                          <div className="flex justify-between">
                            <span className="font-medium">Rate</span>
                            <span>{loan.interest_rate_percent.toFixed(1)}% APR</span>
                          </div>
                          <div className="text-sm text-gray-600">
                            Collateral: {formatBSV(loan.collateral)} BSV ({(loan.collateral_ratio * 100).toFixed(0)}%)
                          </div>
                          <div className="text-sm text-gray-600">
                            Due: {new Date(loan.due_date).toLocaleDateString()}
                          </div>
                          <button
                            onClick={() => handleFundLoan(loan.loan_id)}
                            disabled={loading}
                            style={{
                              width: '100%',
                              background: 'linear-gradient(135deg, #10b981, #059669)',
                              color: 'white',
                              padding: '0.75rem',
                              borderRadius: '0.5rem',
                              border: 'none',
                              cursor: loading ? 'not-allowed' : 'pointer',
                              fontWeight: '600'
                            }}
                          >
                            Fund This Loan
                          </button>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )}
            </div>
          </div>
        </>
      )}

      {connected && currentView === 'history' && (
        <LoanHistory 
          userPaymail={paymail}
          apiBaseUrl="http://localhost:8082"
        />
      )}
      
      {connected && currentView === 'channels' && (
        <PaymentChannels 
          userPaymail={paymail} 
        />
      )}
    </div>
  );
}

export default App;