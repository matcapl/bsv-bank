import React, { useState, useEffect } from 'react';
import './LoanHistory.css';

const LoanHistory = ({ userPaymail, apiBaseUrl = 'http://localhost:8082' }) => {
  const [loans, setLoans] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [viewMode, setViewMode] = useState('all');
  const [selectedLoan, setSelectedLoan] = useState(null);

  useEffect(() => {
    fetchLoans();
  }, [userPaymail]);

  const fetchLoans = async () => {
    setLoading(true);
    setError(null);
    
    try {
      // Fetch loans where user is borrower
      const borrowedResponse = await fetch(
        `${apiBaseUrl}/loans/borrower/${userPaymail}`
      );
      const borrowedLoans = borrowedResponse.ok ? await borrowedResponse.json() : [];

      // Fetch loans where user is lender
      const lentResponse = await fetch(
        `${apiBaseUrl}/loans/lender/${userPaymail}`
      );
      const lentLoans = lentResponse.ok ? await lentResponse.json() : [];

      // Combine and sort by created_at (newest first)
      const allLoans = [...borrowedLoans, ...lentLoans].sort(
        (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
      );

      setLoans(allLoans);
    } catch (err) {
      setError('Failed to fetch loan history');
      console.error('Error fetching loans:', err);
    } finally {
      setLoading(false);
    }
  };

  const filteredLoans = loans.filter(loan => {
    if (viewMode === 'borrowed') return loan.borrower_paymail === userPaymail;
    if (viewMode === 'lent') return loan.lender_paymail === userPaymail;
    return true;
  });

  const getStatusColor = (status) => {
    switch (status) {
      case 'Pending': return '#f59e0b';
      case 'Active': return '#3b82f6';
      case 'Repaid': return '#10b981';
      case 'Liquidated': return '#ef4444';
      default: return '#6b7280';
    }
  };

  const formatDate = (dateString) => {
    if (!dateString) return 'N/A';
    return new Date(dateString).toLocaleDateString('en-US', {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit'
    });
  };

  const formatSatoshis = (amount) => {
    return amount.toLocaleString() + ' sats';
  };

  const calculateTotalDue = (loan) => {
    const interest = Math.floor(loan.amount_satoshis * loan.interest_rate);
    return loan.amount_satoshis + interest;
  };

  const calculateCollateralRatio = (loan) => {
    return ((loan.collateral_satoshis / loan.amount_satoshis) * 100).toFixed(0);
  };

  const getRole = (loan) => {
    if (loan.borrower_paymail === userPaymail) return 'Borrower';
    if (loan.lender_paymail === userPaymail) return 'Lender';
    return 'Unknown';
  };

  const stats = {
    totalBorrowed: loans
      .filter(l => l.borrower_paymail === userPaymail)
      .reduce((sum, l) => sum + l.amount_satoshis, 0),
    totalLent: loans
      .filter(l => l.lender_paymail === userPaymail)
      .reduce((sum, l) => sum + l.amount_satoshis, 0),
    activeBorrowed: loans.filter(l => 
      l.borrower_paymail === userPaymail && l.status === 'Active'
    ).length,
    activeLent: loans.filter(l => 
      l.lender_paymail === userPaymail && l.status === 'Active'
    ).length,
    totalRepaid: loans.filter(l => l.status === 'Repaid').length,
    totalLiquidated: loans.filter(l => l.status === 'Liquidated').length,
  };

  if (loading) {
    return (
      <div className="loan-history">
        <div className="loading">Loading loan history...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="loan-history">
        <div className="error">{error}</div>
        <button onClick={fetchLoans}>Retry</button>
      </div>
    );
  }

  return (
    <div className="loan-history">
      <div className="history-header">
        <h2>Loan History</h2>
        <button onClick={fetchLoans} className="refresh-btn">
          🔄 Refresh
        </button>
      </div>

      {/* Statistics Dashboard */}
      <div className="stats-grid">
        <div className="stat-card">
          <div className="stat-label">Total Borrowed</div>
          <div className="stat-value">{formatSatoshis(stats.totalBorrowed)}</div>
          <div className="stat-subtitle">{stats.activeBorrowed} active</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">Total Lent</div>
          <div className="stat-value">{formatSatoshis(stats.totalLent)}</div>
          <div className="stat-subtitle">{stats.activeLent} active</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">Completed</div>
          <div className="stat-value">{stats.totalRepaid}</div>
          <div className="stat-subtitle">Repaid successfully</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">Liquidated</div>
          <div className="stat-value">{stats.totalLiquidated}</div>
          <div className="stat-subtitle">Collateral seized</div>
        </div>
      </div>

      {/* Filter Tabs */}
      <div className="filter-tabs">
        <button
          className={viewMode === 'all' ? 'active' : ''}
          onClick={() => setViewMode('all')}
        >
          All Loans ({loans.length})
        </button>
        <button
          className={viewMode === 'borrowed' ? 'active' : ''}
          onClick={() => setViewMode('borrowed')}
        >
          Borrowed ({loans.filter(l => l.borrower_paymail === userPaymail).length})
        </button>
        <button
          className={viewMode === 'lent' ? 'active' : ''}
          onClick={() => setViewMode('lent')}
        >
          Lent ({loans.filter(l => l.lender_paymail === userPaymail).length})
        </button>
      </div>

      {/* Loans List */}
      {filteredLoans.length === 0 ? (
        <div className="no-loans">
          <p>No loans found</p>
        </div>
      ) : (
        <div className="loans-list">
          {filteredLoans.map(loan => (
            <div
              key={loan.id}
              className="loan-card"
              onClick={() => setSelectedLoan(loan)}
            >
              <div className="loan-card-header">
                <div className="loan-role-badge" data-role={getRole(loan).toLowerCase()}>
                  {getRole(loan)}
                </div>
                <div
                  className="loan-status-badge"
                  style={{ backgroundColor: getStatusColor(loan.status) }}
                >
                  {loan.status}
                </div>
              </div>

              <div className="loan-card-body">
                <div className="loan-amount">
                  <span className="amount-label">Loan Amount</span>
                  <span className="amount-value">{formatSatoshis(loan.amount_satoshis)}</span>
                </div>

                <div className="loan-details-grid">
                  <div className="detail-item">
                    <span className="detail-label">Interest Rate</span>
                    <span className="detail-value">{(loan.interest_rate * 100).toFixed(2)}%</span>
                  </div>
                  <div className="detail-item">
                    <span className="detail-label">Duration</span>
                    <span className="detail-value">{loan.duration_days} days</span>
                  </div>
                  <div className="detail-item">
                    <span className="detail-label">Collateral</span>
                    <span className="detail-value">{formatSatoshis(loan.collateral_satoshis)}</span>
                  </div>
                  <div className="detail-item">
                    <span className="detail-label">Collateral Ratio</span>
                    <span className="detail-value">{calculateCollateralRatio(loan)}%</span>
                  </div>
                </div>

                <div className="loan-counterparty">
                  {getRole(loan) === 'Borrower' ? (
                    <span>
                      Lender: {loan.lender_paymail || 'Waiting for funding...'}
                    </span>
                  ) : (
                    <span>Borrower: {loan.borrower_paymail}</span>
                  )}
                </div>

                <div className="loan-dates">
                  <span>Created: {formatDate(loan.created_at)}</span>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Loan Detail Modal */}
      {selectedLoan && (
        <div className="modal-overlay" onClick={() => setSelectedLoan(null)}>
          <div className="modal-content" onClick={e => e.stopPropagation()}>
            <div className="modal-header">
              <h3>Loan Details</h3>
              <button className="close-btn" onClick={() => setSelectedLoan(null)}>
                ✕
              </button>
            </div>

            <div className="modal-body">
              <div className="detail-section">
                <h4>Basic Information</h4>
                <div className="detail-row">
                  <span>Loan ID:</span>
                  <span className="mono">{selectedLoan.id}</span>
                </div>
                <div className="detail-row">
                  <span>Status:</span>
                  <span
                    className="status-pill"
                    style={{ backgroundColor: getStatusColor(selectedLoan.status) }}
                  >
                    {selectedLoan.status}
                  </span>
                </div>
                <div className="detail-row">
                  <span>Your Role:</span>
                  <span>{getRole(selectedLoan)}</span>
                </div>
              </div>

              <div className="detail-section">
                <h4>Financial Details</h4>
                <div className="detail-row">
                  <span>Principal Amount:</span>
                  <span>{formatSatoshis(selectedLoan.amount_satoshis)}</span>
                </div>
                <div className="detail-row">
                  <span>Interest Rate:</span>
                  <span>{(selectedLoan.interest_rate * 100).toFixed(2)}%</span>
                </div>
                <div className="detail-row">
                  <span>Interest Amount:</span>
                  <span>
                    {formatSatoshis(
                      Math.floor(selectedLoan.amount_satoshis * selectedLoan.interest_rate)
                    )}
                  </span>
                </div>
                <div className="detail-row highlight">
                  <span>Total Due:</span>
                  <span>{formatSatoshis(calculateTotalDue(selectedLoan))}</span>
                </div>
                <div className="detail-row">
                  <span>Collateral:</span>
                  <span>{formatSatoshis(selectedLoan.collateral_satoshis)}</span>
                </div>
                <div className="detail-row">
                  <span>Collateral Ratio:</span>
                  <span>{calculateCollateralRatio(selectedLoan)}%</span>
                </div>
                <div className="detail-row">
                  <span>Duration:</span>
                  <span>{selectedLoan.duration_days} days</span>
                </div>
              </div>

              <div className="detail-section">
                <h4>Parties</h4>
                <div className="detail-row">
                  <span>Borrower:</span>
                  <span className="mono">{selectedLoan.borrower_paymail}</span>
                </div>
                <div className="detail-row">
                  <span>Lender:</span>
                  <span className="mono">
                    {selectedLoan.lender_paymail || 'Not yet funded'}
                  </span>
                </div>
              </div>

              <div className="detail-section">
                <h4>Timeline</h4>
                <div className="timeline">
                  <div className="timeline-item">
                    <div className="timeline-dot completed"></div>
                    <div className="timeline-content">
                      <div className="timeline-title">Loan Created</div>
                      <div className="timeline-date">{formatDate(selectedLoan.created_at)}</div>
                    </div>
                  </div>

                  {selectedLoan.funded_at && (
                    <div className="timeline-item">
                      <div className="timeline-dot completed"></div>
                      <div className="timeline-content">
                        <div className="timeline-title">Funded</div>
                        <div className="timeline-date">{formatDate(selectedLoan.funded_at)}</div>
                      </div>
                    </div>
                  )}

                  {selectedLoan.repaid_at && (
                    <div className="timeline-item">
                      <div className="timeline-dot completed"></div>
                      <div className="timeline-content">
                        <div className="timeline-title">Repaid</div>
                        <div className="timeline-date">{formatDate(selectedLoan.repaid_at)}</div>
                      </div>
                    </div>
                  )}

                  {selectedLoan.liquidated_at && (
                    <div className="timeline-item">
                      <div className="timeline-dot liquidated"></div>
                      <div className="timeline-content">
                        <div className="timeline-title">Liquidated</div>
                        <div className="timeline-date">{formatDate(selectedLoan.liquidated_at)}</div>
                      </div>
                    </div>
                  )}
                </div>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default LoanHistory;