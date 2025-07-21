import numpy as np
import matplotlib.pyplot as plt
import seaborn as sns
from matplotlib.patches import Rectangle
from matplotlib.widgets import Slider
import pandas as pd

def calculate_dynamic_spread_adjustment(dynamic_spread_factor, dynamic_spread_cap, imbalance_f64):
    """
    Python implementation of the Rust function to calculate dynamic spread adjustment
    
    Args:
        dynamic_spread_factor: negative = slow then fast, positive = fast then slow, zero = linear
        dynamic_spread_cap: maximum distance allowed to move
        imbalance_f64: token value imbalance ratio (-1.0 to 1.0)
    
    Returns:
        (tick_adjustment, fee_tier_adjustment)
    """
    if dynamic_spread_cap == 0 or abs(imbalance_f64) < 1e-10:
        return (0, 0)
    
    # Calculate the dynamic spread adjustment
    if dynamic_spread_factor == 0:
        # Linear movement: f(x) = x * cap
        spread_adjustment = abs(imbalance_f64) * dynamic_spread_cap
    elif dynamic_spread_factor < 0:
        # Slow at first, then faster (exponential curve that starts BELOW linear)
        # f(x) = x^(1+q) * cap where q = |factor|/100
        q = abs(dynamic_spread_factor) / 100.0  # normalize factor (positive)
        x = abs(imbalance_f64)
        exponential_curve = np.power(x, 1.0 + q)
        spread_adjustment = exponential_curve * dynamic_spread_cap
    else:
        # Fast at first, then slower (logarithmic curve that starts ABOVE linear, then goes BELOW)
        # f(x) = (1 - e^(-x*n)) * cap / (1 - e^(-n)) where n = factor/100
        n = dynamic_spread_factor / 100.0  # normalize factor
        x = abs(imbalance_f64)
        if abs(n) < 1e-10:
            # Handle edge case where n approaches 0 (should be linear)
            log_curve = x
        else:
            log_curve = (1.0 - np.exp(-x * n)) / (1.0 - np.exp(-n))
        spread_adjustment = log_curve * dynamic_spread_cap
    
    # Divide by 2 to avoid double counting since both tick adjustment and fee adjustment contribute
    half_adjustment = spread_adjustment / 2.0
    
    # Determine which asset is undersupplied and adjust accordingly
    if imbalance_f64 > 0.0:
        # Token0 dominates, token1 is undersupplied
        # Move tick index down to favor token1, widen spread on token1 side
        tick_adj = -round(half_adjustment)
        fee_adj = round(half_adjustment)
        return (tick_adj, fee_adj)
    elif imbalance_f64 < 0.0:
        # Token1 dominates, token0 is undersupplied
        # Move tick index up to favor token0, widen spread on token0 side
        tick_adj = round(half_adjustment)
        fee_adj = round(half_adjustment)
        return (tick_adj, fee_adj)
    else:
        # Perfectly balanced
        return (0, 0)

def create_comprehensive_visualization():
    """Create a comprehensive visualization of the fee tier adjustment system"""
    
    # Set up the plotting style
    plt.style.use('seaborn-v0_8')
    fig = plt.figure(figsize=(20, 16))
    
    # Define parameter ranges
    imbalance_range = np.linspace(-1.0, 1.0, 100)
    spread_factors = [-100, -50, 0, 50, 100]  # Different curve types
    spread_caps = [50, 100, 200]  # Different maximum adjustments
    
    # 1. Main curves showing different spread factors
    ax1 = plt.subplot(2, 3, 1)
    for factor in spread_factors:
        tick_adjustments = []
        fee_adjustments = []
        
        for imbalance in imbalance_range:
            tick_adj, fee_adj = calculate_dynamic_spread_adjustment(factor, 100, imbalance)
            tick_adjustments.append(tick_adj)
            fee_adjustments.append(fee_adj)
        
        label = f"Factor: {factor}"
        if factor < 0:
            label += " (Exponential)"
        elif factor == 0:
            label += " (Linear)"
        else:
            label += " (Logarithmic)"
            
        ax1.plot(imbalance_range, tick_adjustments, label=f"Tick {label}", linestyle='-', linewidth=2)
        ax1.plot(imbalance_range, fee_adjustments, label=f"Fee {label}", linestyle='--', linewidth=2, alpha=0.7)
    
    ax1.set_xlabel('Imbalance Ratio (-1.0 to 1.0)')
    ax1.set_ylabel('Adjustment (basis points)')
    ax1.set_title('Tick & Fee Adjustments vs Imbalance\n(Cap = 100)')
    ax1.legend(bbox_to_anchor=(1.05, 1), loc='upper left')
    ax1.grid(True, alpha=0.3)
    ax1.axhline(y=0, color='black', linestyle='-', alpha=0.3)
    ax1.axvline(x=0, color='black', linestyle='-', alpha=0.3)
    
    # 2. Effect of different caps
    ax2 = plt.subplot(2, 3, 2)
    for cap in spread_caps:
        total_adjustments = []
        
        for imbalance in imbalance_range:
            tick_adj, fee_adj = calculate_dynamic_spread_adjustment(0, cap, imbalance)  # Linear for simplicity
            total_adjustments.append(abs(tick_adj) + abs(fee_adj))
        
        ax2.plot(imbalance_range, total_adjustments, label=f"Cap: {cap}", linewidth=2)
    
    ax2.set_xlabel('Imbalance Ratio (-1.0 to 1.0)')
    ax2.set_ylabel('Total Adjustment (basis points)')
    ax2.set_title('Effect of Different Spread Caps\n(Linear Factor = 0)')
    ax2.legend()
    ax2.grid(True, alpha=0.3)
    
    # 3. Heatmap showing combined effect
    ax3 = plt.subplot(2, 3, 3)
    factors_range = np.linspace(-100, 100, 20)
    imbalance_heatmap = np.linspace(-1.0, 1.0, 20)
    
    total_adjustment_matrix = np.zeros((len(factors_range), len(imbalance_heatmap)))
    
    for i, factor in enumerate(factors_range):
        for j, imbalance in enumerate(imbalance_heatmap):
            tick_adj, fee_adj = calculate_dynamic_spread_adjustment(int(factor), 100, imbalance)
            total_adjustment_matrix[i, j] = abs(tick_adj) + abs(fee_adj)
    
    im = ax3.imshow(total_adjustment_matrix, cmap='viridis', aspect='auto', 
                    extent=[-1, 1, -100, 100], origin='lower')
    ax3.set_xlabel('Imbalance Ratio')
    ax3.set_ylabel('Dynamic Spread Factor')
    ax3.set_title('Total Adjustment Heatmap\n(Cap = 100)')
    plt.colorbar(im, ax=ax3, label='Total Adjustment (basis points)')
    
    # 4. Curve comparison at different imbalance levels
    ax4 = plt.subplot(2, 3, 4)
    factors_detailed = np.linspace(-100, 100, 50)
    imbalance_levels = [0.25, 0.5, 0.75, 1.0]
    
    for imbalance_level in imbalance_levels:
        adjustments = []
        for factor in factors_detailed:
            tick_adj, fee_adj = calculate_dynamic_spread_adjustment(int(factor), 100, imbalance_level)
            adjustments.append(abs(tick_adj) + abs(fee_adj))
        
        ax4.plot(factors_detailed, adjustments, label=f'Imbalance: {imbalance_level}', linewidth=2)
    
    ax4.set_xlabel('Dynamic Spread Factor')
    ax4.set_ylabel('Total Adjustment (basis points)')
    ax4.set_title('Adjustment vs Spread Factor\n(Cap = 100)')
    ax4.legend()
    ax4.grid(True, alpha=0.3)
    
    # 5. Before/After fee tier visualization
    ax5 = plt.subplot(2, 3, 5)
    
    # Example fee tiers
    base_fee_tiers = [10, 25, 50, 100]  # basis points
    imbalance_example = 0.8  # 80% imbalance
    factor_example = 0  # Linear
    cap_example = 100
    
    tick_adj, fee_adj = calculate_dynamic_spread_adjustment(factor_example, cap_example, imbalance_example)
    adjusted_fee_tiers = [max(0, fee + fee_adj) for fee in base_fee_tiers]
    
    x_pos = np.arange(len(base_fee_tiers))
    width = 0.35
    
    bars1 = ax5.bar(x_pos - width/2, base_fee_tiers, width, label='Original Fee Tiers', alpha=0.7)
    bars2 = ax5.bar(x_pos + width/2, adjusted_fee_tiers, width, label='Adjusted Fee Tiers', alpha=0.7)
    
    ax5.set_xlabel('Fee Tier Index')
    ax5.set_ylabel('Fee (basis points)')
    ax5.set_title(f'Fee Tier Adjustment Example\n(Imbalance: {imbalance_example}, Adjustment: +{fee_adj})')
    ax5.set_xticks(x_pos)
    ax5.set_xticklabels([f'Tier {i+1}' for i in range(len(base_fee_tiers))])
    ax5.legend()
    ax5.grid(True, alpha=0.3)
    
    # Add value labels on bars
    for bar in bars1:
        height = bar.get_height()
        ax5.text(bar.get_x() + bar.get_width()/2., height + 1,
                f'{int(height)}', ha='center', va='bottom', fontsize=10)
    
    for bar in bars2:
        height = bar.get_height()
        ax5.text(bar.get_x() + bar.get_width()/2., height + 1,
                f'{int(height)}', ha='center', va='bottom', fontsize=10)
    
    # 6. Summary table
    ax6 = plt.subplot(2, 3, 6)
    ax6.axis('off')
    
    # Create summary data
    summary_data = []
    test_cases = [
        (-100, 100, 0.5, "Exponential, 50% imbalance"),
        (0, 100, 0.5, "Linear, 50% imbalance"),
        (100, 100, 0.5, "Logarithmic, 50% imbalance"),
        (0, 50, 1.0, "Linear, 100% imbalance, low cap"),
        (0, 200, 1.0, "Linear, 100% imbalance, high cap"),
    ]
    
    for factor, cap, imbalance, description in test_cases:
        tick_adj, fee_adj = calculate_dynamic_spread_adjustment(factor, cap, imbalance)
        summary_data.append([description, f"{tick_adj:+d}", f"{fee_adj:+d}", f"{abs(tick_adj) + abs(fee_adj):.0f}"])
    
    table = ax6.table(cellText=summary_data,
                     colLabels=['Scenario', 'Tick Adj', 'Fee Adj', 'Total'],
                     cellLoc='center',
                     loc='center',
                     bbox=[0, 0, 1, 1])
    
    table.auto_set_font_size(False)
    table.set_fontsize(10)
    table.scale(1, 2)
    
    # Style the table
    for i in range(len(summary_data) + 1):
        for j in range(4):
            if i == 0:  # Header row
                table[(i, j)].set_facecolor('#4CAF50')
                table[(i, j)].set_text_props(weight='bold', color='white')
            else:
                table[(i, j)].set_facecolor('#f0f0f0' if i % 2 == 0 else 'white')
    
    ax6.set_title('Summary of Adjustments\nfor Different Scenarios', pad=20, fontsize=12, weight='bold')
    
    plt.tight_layout()
    plt.show()

def create_interactive_dashboard():
    """Create an interactive dashboard showing real-time adjustments"""
    
    print("=== Dynamic Fee Tier Adjustment Calculator ===\n")
    
    while True:
        try:
            print("Enter parameters (or 'quit' to exit):")
            
            # Get user input
            factor_input = input("Dynamic Spread Factor (-100 to 100, 0=linear): ")
            if factor_input.lower() == 'quit':
                break
            factor = int(factor_input)
            
            cap = int(input("Dynamic Spread Cap (e.g., 100): "))
            imbalance = float(input("Imbalance Ratio (-1.0 to 1.0): "))
            
            # Calculate adjustments
            tick_adj, fee_adj = calculate_dynamic_spread_adjustment(factor, cap, imbalance)
            
            # Display results
            print(f"\n--- Results ---")
            print(f"Tick Index Adjustment: {tick_adj:+d} basis points")
            print(f"Fee Tier Adjustment: {fee_adj:+d} basis points")
            print(f"Total Effect: {abs(tick_adj) + abs(fee_adj):.0f} basis points")
            
            # Show direction
            if imbalance > 0:
                print(f"Direction: Token0 dominates → favoring Token1")
            elif imbalance < 0:
                print(f"Direction: Token1 dominates → favoring Token0")
            else:
                print(f"Direction: Balanced")
            
            # Show curve type
            if factor < 0:
                print(f"Curve Type: Exponential (slow then fast)")
            elif factor == 0:
                print(f"Curve Type: Linear")
            else:
                print(f"Curve Type: Logarithmic (fast then slow)")
            
            print("-" * 50)
            
        except ValueError:
            print("Invalid input. Please enter numeric values.")
        except Exception as e:
            print(f"Error: {e}")

def create_dynamic_tick_visualization():
    """Create a dynamic visualization showing tick index movement with sliders"""
    
    # Set up the figure with two subplots
    fig = plt.figure(figsize=(16, 10))
    
    # Main plot for tick visualization
    ax1 = plt.subplot(1, 2, 1)
    plt.subplots_adjust(bottom=0.35, left=0.1, right=0.95)
    
    # Secondary plot for curve comparison
    ax2 = plt.subplot(1, 2, 2)
    
    # Initial parameters
    initial_factor = 0
    initial_cap = 100
    initial_imbalance = 0.5
    initial_base_spread = 100  # Reduced from 200 for better low-value resolution

    # Calculate initial adjustments
    tick_adj, fee_adj = calculate_dynamic_spread_adjustment(initial_factor, initial_cap, initial_imbalance)
    
    # Set up the visualization range
    max_spread = 500  # Maximum spread for visualization
    
    # === MAIN PLOT (ax1) ===
    ax1.set_xlim(-1, 1)
    ax1.set_ylim(-max_spread, max_spread)
    ax1.set_xlabel('Position', fontsize=12)
    ax1.set_ylabel('Tick Index (basis points)', fontsize=12)
    ax1.set_title('Dynamic Tick Index Adjustment', fontsize=14, fontweight='bold')
    
    # Add grid and center line
    ax1.grid(True, alpha=0.3)
    ax1.axhline(y=0, color='black', linewidth=2, label='Center (0)')
    
    # Initialize plot elements - FIXED: bounds should be static based on base spread only
    upper_bound_line = ax1.axhline(y=initial_base_spread/2, color='red', linewidth=3, alpha=0.7, label='Upper Bound (Static)')
    lower_bound_line = ax1.axhline(y=-initial_base_spread/2, color='red', linewidth=3, alpha=0.7, label='Lower Bound (Static)')
    tick_index_line = ax1.axhline(y=tick_adj, color='blue', linewidth=2, alpha=0.6, label='Current Tick Index')  # Made less bold
    
    # Add markers for better visibility
    upper_marker = ax1.plot(0, initial_base_spread/2, 'ro', markersize=10)[0]
    lower_marker = ax1.plot(0, -initial_base_spread/2, 'ro', markersize=10)[0]
    tick_marker = ax1.plot(0, tick_adj, 'bo', markersize=10, alpha=0.7)[0]  # Made less bold
    
    # Add text annotations
    upper_text = ax1.text(0.5, initial_base_spread/2 + 20, f'Upper: +{initial_base_spread/2:.0f}', 
                         fontsize=11, ha='center', bbox=dict(boxstyle="round,pad=0.3", facecolor="red", alpha=0.3))
    lower_text = ax1.text(0.5, -initial_base_spread/2 - 20, f'Lower: -{initial_base_spread/2:.0f}', 
                         fontsize=11, ha='center', bbox=dict(boxstyle="round,pad=0.3", facecolor="red", alpha=0.3))
    tick_text = ax1.text(-0.5, tick_adj + 20, f'Tick: {tick_adj:+.0f}', 
                        fontsize=11, ha='center', bbox=dict(boxstyle="round,pad=0.3", facecolor="blue", alpha=0.3))
    
    # Add spread visualization
    spread_patch = Rectangle((-0.8, -initial_base_spread/2), 1.6, initial_base_spread, 
                           facecolor='yellow', alpha=0.2, label='Active Spread')
    ax1.add_patch(spread_patch)
    
    # Add legend
    ax1.legend(loc='upper left', bbox_to_anchor=(0, 1))
    
    # === SECONDARY PLOT (ax2) ===
    ax2.set_xlim(-1, 1)
    ax2.set_ylim(-200, 200)
    ax2.set_xlabel('Imbalance Ratio', fontsize=12)
    ax2.set_ylabel('Tick Adjustment', fontsize=12)
    ax2.set_title('Curve Comparison', fontsize=14, fontweight='bold')
    ax2.grid(True, alpha=0.3)
    ax2.axhline(y=0, color='black', linewidth=1, alpha=0.5)
    ax2.axvline(x=0, color='black', linewidth=1, alpha=0.5)
    
    # Initialize curve plots
    imbalance_range = np.linspace(-1, 1, 100)
    
    # Calculate curves for comparison - keep only linear as reference
    linear_curve = []
    linear_fee_curve = []
    current_curve = []
    current_fee_curve = []
    
    for imb in imbalance_range:
        tick_linear, fee_linear = calculate_dynamic_spread_adjustment(0, initial_cap, imb)
        tick_current, fee_current = calculate_dynamic_spread_adjustment(initial_factor, initial_cap, imb)
        
        linear_curve.append(tick_linear)
        linear_fee_curve.append(fee_linear)
        current_curve.append(tick_current)
        current_fee_curve.append(fee_current)
    
    # Plot only linear reference and current curve
    line_linear, = ax2.plot(imbalance_range, linear_curve, 'gray', linewidth=2, alpha=0.5, label='Linear Tick (Factor=0)')
    line_linear_fee, = ax2.plot(imbalance_range, linear_fee_curve, 'gray', linewidth=2, alpha=0.5, linestyle='--', label='Linear Fee (Factor=0)')
    
    # Current curve (will be updated dynamically)
    line_current, = ax2.plot(imbalance_range, current_curve, 'blue', linewidth=3, label=f'Current Tick (Factor={initial_factor})')
    line_current_fee, = ax2.plot(imbalance_range, current_fee_curve, 'blue', linewidth=3, linestyle='--', label=f'Current Fee (Factor={initial_factor})')
    
    # Current point indicator
    current_point = ax2.plot(initial_imbalance, tick_adj, 'ko', markersize=8, label='Current Tick')[0]
    current_fee_point = ax2.plot(initial_imbalance, fee_adj, 'ko', markersize=8, marker='s', label='Current Fee')[0]
    
    ax2.legend()
    
    # Create sliders
    slider_height = 0.03
    slider_spacing = 0.05
    
    # Imbalance slider
    ax_imbalance = plt.axes([0.15, 0.25, 0.65, slider_height])
    slider_imbalance = Slider(ax_imbalance, 'Imbalance Ratio', -1.0, 1.0, 
                             valinit=initial_imbalance, valfmt='%.2f')
    slider_imbalance.label.set_fontsize(10)
    
    # Dynamic spread factor slider - Extended range for more extreme curve options
    ax_factor = plt.axes([0.15, 0.25 - slider_spacing, 0.65, slider_height])
    slider_factor = Slider(ax_factor, 'Spread Factor', -1000, 1000, 
                          valinit=initial_factor, valfmt='%.0f')
    slider_factor.label.set_fontsize(10)
    
    # Dynamic spread cap slider
    ax_cap = plt.axes([0.15, 0.25 - 2*slider_spacing, 0.65, slider_height])
    slider_cap = Slider(ax_cap, 'Spread Cap', 0, 300, 
                       valinit=initial_cap, valfmt='%.0f')
    slider_cap.label.set_fontsize(10)
    
    # Base spread slider - Start from 0 for better low-value resolution
    ax_base_spread = plt.axes([0.15, 0.25 - 3*slider_spacing, 0.65, slider_height])
    slider_base_spread = Slider(ax_base_spread, 'Base Spread', 0, 500, 
                               valinit=initial_base_spread, valfmt='%.0f')
    slider_base_spread.label.set_fontsize(10)
    
    # Add info text - moved to bottom right to avoid blocking other elements
    info_text = ax1.text(0.98, 0.02, 
                        f'Curve Type: Linear\nTick Adj: {tick_adj:+.0f}\nFee Adj: {fee_adj:+.0f}', 
                        transform=ax1.transAxes, fontsize=10, verticalalignment='bottom', horizontalalignment='right',
                        bbox=dict(boxstyle="round,pad=0.5", facecolor="lightgray", alpha=0.8))
    
    def update(val):
        # Get current slider values
        imbalance = slider_imbalance.val
        factor = int(slider_factor.val)
        cap = int(slider_cap.val)
        base_spread = slider_base_spread.val
        
        # Calculate new adjustments
        tick_adj, fee_adj = calculate_dynamic_spread_adjustment(factor, cap, imbalance)
        
        # Base bounds (static when imbalance = 0)
        base_upper = base_spread / 2
        base_lower = -base_spread / 2
        
        # FIXED: Bounds can move by the full fee adjustment amount based on imbalance direction
        if imbalance > 0:
            # Token0 dominates → Token1 is undersupplied
            # Move lower bound down by the fee adjustment amount
            effective_upper = base_upper  # Upper bound stays static
            effective_lower = base_lower - fee_adj  # Lower bound moves down
            
        elif imbalance < 0:
            # Token1 dominates → Token0 is undersupplied  
            # Move upper bound up by the fee adjustment amount
            effective_upper = base_upper + fee_adj  # Upper bound moves up
            effective_lower = base_lower  # Lower bound stays static
            
        else:
            # Balanced - no movement
            effective_upper = base_upper
            effective_lower = base_lower
        
        # FIXED: Tick index should move at half the speed of the bounds movement
        # Start from the center of the ORIGINAL spread
        original_center = (base_upper + base_lower) / 2  # This is always 0 for symmetric spreads
        
        # Calculate tick position: move at half the speed of bounds movement
        if imbalance > 0:
            # Lower bound moved down by fee_adj, so tick moves down by fee_adj/2
            tick_position = original_center - (fee_adj / 2.0)
        elif imbalance < 0:
            # Upper bound moved up by fee_adj, so tick moves up by fee_adj/2  
            tick_position = original_center + (fee_adj / 2.0)
        else:
            # Balanced - tick stays at center
            tick_position = original_center
        
        # Ensure tick index is ALWAYS between the effective bounds with proper margins
        # Use smaller margins for very small spreads to maintain visibility
        spread_size = effective_upper - effective_lower
        if spread_size > 0:
            margin = min(5, spread_size * 0.02)  # 2% margin or 5bp, whichever is smaller
        else:
            margin = 1  # Minimum margin for zero spread
        tick_position = max(effective_lower + margin, min(effective_upper - margin, tick_position))
        
        # Update the main plot elements
        upper_bound_line.set_ydata([effective_upper, effective_upper])
        lower_bound_line.set_ydata([effective_lower, effective_lower])
        tick_index_line.set_ydata([tick_position, tick_position])
        
        # Update markers
        upper_marker.set_ydata([effective_upper])
        lower_marker.set_ydata([effective_lower])
        tick_marker.set_ydata([tick_position])
        
        # Update spread patch to show effective spread
        spread_patch.set_y(effective_lower)
        spread_patch.set_height(effective_upper - effective_lower)
        
        # Change color based on which bound is moving
        if imbalance > 0 and fee_adj > 0:
            spread_patch.set_facecolor('lightcoral')  # Lower bound moved down
            spread_patch.set_alpha(0.4)
        elif imbalance < 0 and fee_adj > 0:
            spread_patch.set_facecolor('lightblue')  # Upper bound moved up
            spread_patch.set_alpha(0.4)
        else:
            spread_patch.set_facecolor('yellow')  # No significant movement
            spread_patch.set_alpha(0.2)
        
        # Calculate correct fee tier: half the spread + half the movement of expensive side
        if not hasattr(update, 'fee_display'):
            # Single fee tier text display
            update.fee_display = ax1.text(0.75, 0, f'Fee Tier: 0bp', ha='center', va='center', 
                                        fontsize=12, fontweight='bold',
                                        bbox=dict(boxstyle="round,pad=0.5", facecolor="lightgreen", alpha=0.8))
        
        # Calculate fee tier correctly:
        # Fee tier = half the spread + half the movement of the side becoming more expensive
        half_spread = base_spread / 2.0
        
        if cap == 0:
            # If spread cap is zero, fee tier is just half the spread
            calculated_fee_tier = half_spread
        elif abs(imbalance) < 1e-10:
            # If perfectly balanced, fee tier is just half the spread
            calculated_fee_tier = half_spread
        else:
            # If there's imbalance, add half the movement of the expensive side
            # The expensive side is the undersupplied token
            if imbalance > 0:
                # Token1 is undersupplied (more expensive), movement = fee_adj
                half_movement = fee_adj / 2.0
            else:
                # Token0 is undersupplied (more expensive), movement = fee_adj  
                half_movement = fee_adj / 2.0
            
            calculated_fee_tier = half_spread + half_movement
        
        calculated_fee_tier = max(0, calculated_fee_tier)  # Don't go negative
        
        # Update fee display text and color
        update.fee_display.set_text(f'Fee Tier: {calculated_fee_tier:.0f}bp')
        
        # Color based on whether fee tier increased from base
        if calculated_fee_tier > half_spread:
            update.fee_display.set_bbox(dict(boxstyle="round,pad=0.5", facecolor="lightcoral", alpha=0.8))
        elif calculated_fee_tier < half_spread:
            update.fee_display.set_bbox(dict(boxstyle="round,pad=0.5", facecolor="lightblue", alpha=0.8))
        else:
            update.fee_display.set_bbox(dict(boxstyle="round,pad=0.5", facecolor="lightgreen", alpha=0.8))
        
        # Update text annotations
        upper_text.set_position((0.5, effective_upper + 20))
        if imbalance < 0 and fee_adj > 0:
            upper_text.set_text(f'Upper: +{effective_upper:.0f} (MOVED +{fee_adj:.0f})')
            upper_text.set_bbox(dict(boxstyle="round,pad=0.3", facecolor="lightblue", alpha=0.7))
        else:
            upper_text.set_text(f'Upper: +{effective_upper:.0f}')
            upper_text.set_bbox(dict(boxstyle="round,pad=0.3", facecolor="red", alpha=0.3))
        
        lower_text.set_position((0.5, effective_lower - 20))
        if imbalance > 0 and fee_adj > 0:
            lower_text.set_text(f'Lower: {effective_lower:.0f} (MOVED -{fee_adj:.0f})')
            lower_text.set_bbox(dict(boxstyle="round,pad=0.3", facecolor="lightcoral", alpha=0.7))
        else:
            lower_text.set_text(f'Lower: {effective_lower:.0f}')
            lower_text.set_bbox(dict(boxstyle="round,pad=0.3", facecolor="red", alpha=0.3))
        
        tick_text.set_position((-0.5, tick_position + 20))
        tick_text.set_text(f'Tick: {tick_position:+.0f}')
        
        # Remove the effective bounds lines (no longer needed)
        if hasattr(update, 'effective_upper_line'):
            update.effective_upper_line.set_visible(False)
            update.effective_lower_line.set_visible(False)
        
        # Update secondary plot
        current_curve = []
        current_fee_curve = []
        for imb in imbalance_range:
            tick_current, fee_current = calculate_dynamic_spread_adjustment(factor, cap, imb)
            current_curve.append(tick_current)
            current_fee_curve.append(fee_current)
        
        # Update the current curve (we'll use the exponential line to show current settings)
        line_current.set_ydata(current_curve)
        line_current.set_label(f'Current Tick (Factor={factor})')
        
        # Update the current fee curve
        line_current_fee.set_ydata(current_fee_curve)
        line_current_fee.set_label(f'Current Fee (Factor={factor})')
        
        # IMPORTANT: Update the legend to reflect new labels
        ax2.legend()
        
        # Update current point
        current_point.set_xdata([imbalance])
        current_point.set_ydata([tick_adj])  # Use actual tick adjustment, not constrained position
        
        # Update current fee point
        current_fee_point.set_xdata([imbalance])
        current_fee_point.set_ydata([fee_adj])
        
        # Update info text
        curve_type = "Linear"
        if factor < 0:
            curve_type = "Exponential (slow→fast)"
        elif factor > 0:
            curve_type = "Logarithmic (fast→slow)"
        
        direction = "Balanced"
        bound_movement = "No movement"
        if imbalance > 0:
            direction = "Token0 dominates → favor Token1"
            if fee_adj > 0:
                bound_movement = f"Lower bound moved down by {fee_adj:.0f}bp"
        elif imbalance < 0:
            direction = "Token1 dominates → favor Token0"
            if fee_adj > 0:
                bound_movement = f"Upper bound moved up by {fee_adj:.0f}bp"
        
        # Show spread information and fee tier calculation
        spread_info = f"Spread: {effective_upper - effective_lower:.0f}bp (base: {base_spread:.0f}bp)"
        center_info = f"Center: {original_center:.0f}bp"
        fee_tier_info = f"Fee Tier: {calculated_fee_tier:.0f}bp = {half_spread:.0f}bp (½ spread)"
        
        if abs(imbalance) > 1e-10 and cap > 0:
            half_movement = fee_adj / 2.0
            fee_tier_info += f" + {half_movement:.0f}bp (½ movement)"
        
        info_text.set_text(f'Curve Type: {curve_type}\nDirection: {direction}\nTick Position: {tick_position:+.0f}bp\n{center_info}\nFee Adj: {fee_adj:+.0f}bp\n{fee_tier_info}\nBound Movement: {bound_movement}\n{spread_info}')
        
        # Redraw
        fig.canvas.draw_idle()
    
    # Connect sliders to update function
    slider_imbalance.on_changed(update)
    slider_factor.on_changed(update)
    slider_cap.on_changed(update)
    slider_base_spread.on_changed(update)
    
    # Add instructions
    plt.figtext(0.5, 0.02, 
                'Left: Blue tick constrained between bounds, fee tier bars (right side) show increasing fees. Right: Curve comparison.',
                ha='center', fontsize=10, style='italic')
    
    plt.show()

if __name__ == "__main__":
    create_dynamic_tick_visualization() 